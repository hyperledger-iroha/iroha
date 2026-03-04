---
lang: ar
direction: rtl
source: docs/portal/docs/soranet/pq-rollout-plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: خطة الطرح PQ
العنوان: دليل التشغيل SNNet-16G لما بعد الكم
Sidebar_label: خطة طرح PQ
الوصف: SoraNet المصافحة الهجينة X25519+ML-KEM عبارة عن مرحلات الكناري الافتراضية، ويعمل العملاء ومجموعات SDK على الترويج لعمل الإجراءات.
---

:::ملاحظة المصدر الكنسي
هذه هي الصفحة `docs/source/soranet/pq_rollout_plan.md`. إذا لم يتم تعيين وثائق برانا للتقاعد، فلا داعي للمزامنة.
:::

يتم تنفيذ نقل SNNet-16G SoraNet في مرحلة ما بعد الكم. `rollout_phase` مقابض المشغلات التي تعمل على تنسيق الترويج الحتمي الموجود في المرحلة A متطلبات الحماية التي توفر تغطية أغلبية المرحلة B والمرحلة C الصارمة لوضعية PQ، حيث يتم تحرير سطح JSON/TOML الخام.

يوجد في غلاف كتاب اللعب ما يلي:

- تعريفات المرحلة ومقابض التكوين الجديدة (`sorafs.gateway.rollout_phase`، `sorafs.rollout_phase`) وهي قاعدة بيانات سلكية (`crates/iroha_config/src/parameters/actual.rs:2230`، `crates/iroha/src/config/user.rs:251`).
- تعيين علامات SDK وCLI لطرح العميل وتتبعه.
- توقعات جدولة الترحيل/العميل ولوحات معلومات الحوكمة التي تتضمن بوابة الترويج (`dashboards/grafana/soranet_pq_ratchet.json`).
- مراجع خطافات التراجع ودليل التدريب على الحرائق ([دليل تشغيل السقاطة PQ](./pq-ratchet-runbook.md)).

## خريطة المرحلة| `rollout_phase` | مرحلة عدم الكشف عن هويته الفعالة | التأثير الافتراضي | الاستخدام النموذجي |
|-----------------|-------------------------------------------|----------------|---------------|
| `canary` | `anon-guard-pq` (المرحلة أ) | لا بد من إنشاء دائرة ممتدة من حارس PQ. | خط الأساس وابتدائی الكناري ہفتے۔ |
| `ramp` | `anon-majority-pq` (المرحلة ب) | اختيار مرحلات PQ للتحيز الجانبي >= تغطية مزدوجة؛ المرحلات الكلاسيكية الاحتياطية | جزر الكناري تتابع كل منطقة على حدة؛ تبديل معاينة SDK ۔ |
| `default` | `anon-strict-pq` (المرحلة ج) | تعمل دوائر PQ فقط على فرض الإنذارات القوية والتخفيضية. | سيتم استكمال القياس عن بعد وتسجيل الحوكمة بعد الترقية الأخيرة. |

إذا كان السطح صريحًا `anonymity_policy`، فسيتم تعيينه أيضًا على أنه مكون لتجاوز المرحلة. لا يمكن استخدام المرحلة الصريحة بقيمة `rollout_phase` لتأجيل وتجميع مشغلي البيئة في مرحلة الوجه المزدوج ويرث العملاء الكري.

## مرجع التكوين

### المنسق (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

يعمل وقت تشغيل أداة تحميل المنسق على حل المرحلة الاحتياطية (`crates/sorafs_orchestrator/src/lib.rs:2229`) و`sorafs_orchestrator_policy_events_total` و`sorafs_orchestrator_pq_ratio_*` للسطح. يحتوي `docs/examples/sorafs_rollout_stage_b.toml` و`docs/examples/sorafs_rollout_stage_c.toml` على مقتطفات جاهزة للتطبيق.

### عميل الصدأ / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
````iroha::Client` سجل المرحلة المحللة كرتا ہے (`crates/iroha/src/client.rs:2315`) تلك الأوامر المساعدة (مثال کان پر `iroha_cli app sorafs fetch`) المرحلة موجودة كسياسة إخفاء الهوية الافتراضية کے ساتھ تقرير كر سکیں.

## الأتمتة

يقوم مساعدو `cargo xtask` بجدولة عملية الإنشاء والتقاط العناصر تلقائيًا.

1. **الجدول الزمني الإقليمي ينشئ کریں**

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

   الفترات `s`، `m`، `h`، أو `d` اللاحقة مقبولة کرتے ہیں. ينبعث كل من `artifacts/soranet_pq_rollout_plan.json` وملخص Markdown (`artifacts/soranet_pq_rollout_plan.md`) من بطاقة ويطلب التغيير وهو ما يحدث الآن.

2. **حفر القطع الأثرية والتوقيعات ثم التقاطها**

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

   قم بإدارة ملف `artifacts/soranet_pq_rollout/<timestamp>_<label>/` من بطاقة البطاقة، وهو عبارة عن قطعة أثرية لـ BLAKE3 تهضم بطاقة الحوسبة، و`rollout_capture.json` هي الأخرى. البيانات الوصفية والحمولة النافعة لتوقيع Ed25519 تشمل ہوتا ہے۔ هذا هو المفتاح الخاص الذي يستخدم علامة محضر التدريب على مكافحة الحرائق، وهو عبارة عن مفتاح حوكمة جلدي للتحقق من صحة سكاك.

## مصفوفة علم SDK وCLI| السطح | الكناري (المرحلة أ) | المنحدر (المرحلة ب) | الافتراضي (المرحلة ج) |
|---------|------------------|----------------|-------------------|
| جلب `sorafs_cli` | `--anonymity-policy stage-a` أو المرحلة پر الانقطاع | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| التكوين المنسق JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| تكوين عميل الصدأ (`iroha.toml`) | `rollout_phase = "canary"` (افتراضي) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| `iroha_cli` الأوامر الموقعة | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| جافا/أندرويد `GatewayFetchOptions` | `setRolloutPhase("canary")`، اختياري `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")`، اختياري `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")`، اختياري `.ANON_STRICT_PQ` |
| مساعدين منسق جافا سكريبت | `rolloutPhase: "canary"` أو `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| بايثون `fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
| سويفت `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

يقوم تمام SDK بتبديل محلل المرحلة الخاص بخريطة استخدام المنسق الخاص به (`crates/sorafs_orchestrator/src/lib.rs:365`)، وذلك لمرحلة تكوين عمليات النشر متعددة اللغات التي ستستمر خطوة القفل.

## قائمة مراجعة جدولة الكناري

1. **التجربة المبدئية (T ناقص أسبوعين)**- تأكيد تسجيل المرحلة أ معدل انقطاع التيار الكهربائي بنسبة  = 70% ہے (`sorafs_orchestrator_pq_candidate_ratio`).
   - الجدول الزمني لفتحة مراجعة الحوكمة للموافقة على نافذة الكناري.
   - تحديث التدريج `sorafs.gateway.rollout_phase = "ramp"` (إعادة نشر منسق JSON) وخط أنابيب الترويج للتشغيل الجاف.

2. **تتابع الكناري (يوم T)**

   - في أي وقت من الأوقات تقوم أي منطقة بالترويج للكريات، `rollout_phase = "ramp"` للمنسق وبيانات التتابع المشاركة في مجموعة الكرتے ہوئے.
   - لوحة معلومات PQ Ratchet لـ "أحداث السياسة لكل نتيجة" و"معدل انقطاع التيار الكهربائي" لحماية ذاكرة التخزين المؤقت TTL لشاشة المراقبة (لوحة المعلومات ولوحة الطرح).
   - تدقيق تخزين اللقطات `sorafs_cli guard-directory fetch` أعلاه وما بعدها.

3. ** كناري العميل/SDK (T بالإضافة إلى أسبوع واحد)**

   - تعمل تكوينات العميل على تجاوز `rollout_phase = "ramp"` أو مجموعات SDK المختارة التي تتجاوز `stage-b`.
   - التقاط اختلافات القياس عن بعد (`sorafs_orchestrator_policy_events_total` و `client_id` و `region` لرقم حساب المجموعة) وقم بإرفاق سجل حوادث الطرح.

4. **العرض الترويجي الافتراضي (T بالإضافة إلى 3 أسابيع)**

   - يتم تدوير تسجيل الخروج للحوكمة بعد تكوينات المنسق والعميل `rollout_phase = "default"` للتبديل وقائمة التحقق من الاستعداد الموقعة لإصدار المصنوعات.

## قائمة مراجعة الحوكمة والأدلة| تغير المرحلة | بوابة الترويج | حزمة الأدلة | لوحات المعلومات والتنبيهات |
|--------------|----------------|---------------------------------|-----|
| كناري -> المنحدر *(معاينة المرحلة ب)* | معدل انقطاع الطاقة للمرحلة A 14 دن = 0.7 في المنطقة التي تم الترويج لها، التحقق من تذكرة Argon2 p95  الافتراضي *(فرض المرحلة C)* | احتراق القياس عن بعد لـ SN16 لمدة 30 يومًا مكمل، `sn16_handshake_downgrade_total` خط الأساس پر مسطح، كناري العميل ے دوران `sorafs_orchestrator_brownouts_total` صفر، والوكيل لتبديل سجل التدريب. | نسخة `sorafs_cli proxy set-mode --mode gateway|direct`، إخراج `promtool test rules dashboards/alerts/soranet_handshake_rules.yml`، سجل `sorafs_cli guard-directory verify`، وحزمة `cargo xtask soranet-rollout-capture --label default` الموقعة. | تم توثيق لوحة PQ Ratchet وألواح خفض مستوى SN16 `docs/source/sorafs_orchestrator_rollout.md` و`dashboards/grafana/soranet_privacy_metrics.json`. || خفض رتبة الطوارئ / الاستعداد للتراجع | قم بتشغيل زر التشغيل عند ارتفاع عدادات الرجوع إلى إصدار أقدم، أو فشل التحقق من دليل الحراسة، أو `/policy/proxy-toggle` buffer مسلسل أحداث الرجوع إلى إصدار أقدم. | `docs/source/ops/soranet_transport_rollback.md` قائمة التحقق، `sorafs_cli guard-directory import` / `guard-cache prune` السجلات، `cargo xtask soranet-rollout-capture --label rollback`، تذاكر الأحداث، وقوالب الإشعارات. | `dashboards/grafana/soranet_pq_ratchet.json`، `dashboards/grafana/soranet_privacy_metrics.json`، وحزم التنبيه دونوں (`dashboards/alerts/soranet_handshake_rules.yml`، `dashboards/alerts/soranet_privacy_rules.yml`). |

- تم إنشاء `rollout_capture.json` من خلال `artifacts/soranet_pq_rollout/<timestamp>_<label>/`، كما تحتوي حزم الإدارة على لوحة النتائج وتتبعات promtool والخلاصات الشاملة.
- الأدلة التي تم تحميلها (دقائق PDF، حزمة الالتقاط، لقطات الحراسة) حيث يهضم SHA256 دقائق الترويج التي يتم إرفاقها بموافقات البرلمان على مجموعة التدريج التي يتم إعادة تشغيلها مرة أخرى.
- تحتوي التذكرة الترويجية على خطة قياس عن بعد تعمل بشكل ثابت على `docs/source/soranet/snnet16_telemetry_plan.md` ومفردات خفض المستوى وعتبات التنبيه للمصدر الأساسي.

## تحديثات لوحة المعلومات والقياس عن بعد

`dashboards/grafana/soranet_pq_ratchet.json` لوحة التعليقات التوضيحية لـ "خطة الطرح" والتي تم ربطها بدليل التشغيل وربطها بالمرحلة الحالية واستعراضها ومراجعة الإدارة للمرحلة النشطة لفحص اللعبة. وصف اللوحة مقابض التكوين لتبديل المزامنة المستمرة.

تنبيهات حول القواعد الموجودة `stage` استخدام تسمية الكناري والمراحل الافتراضية في حدود السياسة تؤدي إلى تشغيل (`dashboards/alerts/soranet_handshake_rules.yml`).## خطافات التراجع

### الافتراضي -> المنحدر (المرحلة ج -> المرحلة ب)

1. `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` هو منسق يخفض رتبة البطاقة (وتكوينات SDK تعمل على مرآة المرحلة) كما يتم إعادة تشغيل أسطول المرحلة B.
2. يوفر `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"` للعملاء ملف تعريف نقل آمن للملفات الشخصية، والتقاط النص من خلال `/policy/proxy-toggle` لسير عمل المعالجة القابل للتدقيق.
3. `cargo xtask soranet-rollout-capture --label rollback-default` اختلافات دليل الحماية وإخراج promtool ولقطات شاشة لوحة المعلومات `artifacts/soranet_pq_rollout/` أرشيف کیا جا سکے.

### المنحدر -> الكناري (المرحلة ب -> المرحلة أ)

1. يستمر الترويج في التقاط لقطة دليل الحراسة `sorafs_cli guard-directory import --guard-directory guards.json` لاستيراد بطاقة الاستيراد و`sorafs_cli guard-directory verify` مرة أخرى، حيث تشتمل حزمة التخفيض على تجزئات.
2. تتضمن تكوينات المنسق والعميل `rollout_phase = "canary"` تعيين التجاوز (أو تجاوز `anonymity_policy stage-a`) والإعداد [PQratchet runbook](./pq-ratchet-runbook.md) وتكرار مثقاب السقاطة PQ لإثبات خط الأنابيب المنخفض.
3. تم تحديث لقطات شاشة القياس عن بعد PQ Ratchet وSN16 ونتائج التنبيه التي يتم إرفاقها بسجل الحوادث وإخطار الإدارة.

### تذكير الدرابزين

- بالإضافة إلى خفض الرتبة `docs/source/ops/soranet_transport_rollback.md` قم بالإشارة إلى التدرج والتخفيف المؤقت لمتعقب الطرح `TODO:` الذي يقوم بتسجيل المتابعة والمتابعة.
- `dashboards/alerts/soranet_handshake_rules.yml` و`dashboards/alerts/soranet_privacy_rules.yml` يتم التراجع عنهما مرة أخرى وبعد ذلك تغطي `promtool test rules` ميزة التنبيه بحزمة التقاط الانجراف التي تثبت المستند.