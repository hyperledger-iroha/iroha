---
lang: ar
direction: rtl
source: docs/portal/docs/soranet/pq-rollout-plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: خطة الطرح PQ
العنوان: نشر لعبة SNNet-16G
Sidebar_label: خطة طرح PQ
الوصف: عملية إنتاج هجين X25519+مصافحة ML-KEM SoraNet من الكناري إلى الوضع الافتراضي للمرحلات والعملاء ومجموعات SDK.
---

:::note Канонический источник
:::

SNNet-16G يكمل طرح ما بعد الكم للنقل SoraNet. تتيح المقابض `rollout_phase` للمشغل تنسيق تحديد الترقية من المنطقة إلى المرحلة A ومتطلبات الحراسة لتغطية الأغلبية للمرحلة B والمرحلة C ووضعية PQ الصارمة بدون تعديل يستخدم JSON/TOML لكل شيء.

تم إنشاء كتاب اللعب هذا:

- المرحلة المقترحة ومقابض التكوين الجديدة (`sorafs.gateway.rollout_phase`، `sorafs.rollout_phase`) في قاعدة التعليمات البرمجية (`crates/iroha_config/src/parameters/actual.rs:2230`، `crates/iroha/src/config/user.rs:251`).
- تعيين إشارات SDK وCLI التي يمكن لأي عميل متابعة الطرح.
- مراقبة جدولة الكناري للترحيل/العميل ولوحات التحكم الخاصة بالحوكمة، والتي تتضمن ترويج البوابة (`dashboards/grafana/soranet_pq_ratchet.json`).
- خطافات التراجع وملحقات دليل التدريب على الحرائق ([دليل تشغيل السقاطة PQ](./pq-ratchet-runbook.md)).

## بطاقة فاز| `rollout_phase` | مرحلة عدم الكشف عن هويته الفعالة | تأثير الترطيب | الاستخدام النموذجي |
|-----------------|-------------------------------------------|----------------|---------------|
| `canary` | `anon-guard-pq` (المرحلة أ) | تحتاج إلى الحد الأدنى من حماية PQ للدائرة، أثناء ترقية السطح. | خط الأساس وذرات الكناري. |
| `ramp` | `anon-majority-pq` (المرحلة ب) | قم بتغيير الاختيار في مرحلات PQ الأساسية لـ >= التغطية الثلاث؛ المرحلات الكلاسيكية تتميز بالاحتياط. | جزر الكناري التتابع الإقليمي. تبديل معاينة SDK. |
| `default` | `anon-strict-pq` (المرحلة ج) | استخدم دوائر PQ فقط واستخدم إنذارات الرجوع إلى إصدار أقدم. | تم الترويج النهائي بعد تحسين القياس عن بعد وحوكمة تسجيل الخروج. |

إذا تم الانتهاء أيضًا من إضافة `anonymity_policy` إلى المرحلة المسبقة لهذا المكون. النتيجة: مرحلة التأجيل الجديدة - تبدأ من `rollout_phase` بحيث يمكن للمشغلين إيقاف مرحلة واحدة من وقت لآخر وتاريخ العملاء تخلص من نفسك.

## التكوينات المرجعية

### المنسق (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

يقوم مُنسق المُحمل بتغيير المرحلة الاحتياطية خلال فترة التشغيل (`crates/sorafs_orchestrator/src/lib.rs:2229`) ويعلن عنها من خلال `sorafs_orchestrator_policy_events_total` و`sorafs_orchestrator_pq_ratio_*`. سم. `docs/examples/sorafs_rollout_stage_b.toml` و`docs/examples/sorafs_rollout_stage_c.toml` للمقتطفات التالية.

### عميل الصدأ / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
````iroha::Client` المرحلة الثانية من المرحلة المصاحبة (`crates/iroha/src/client.rs:2315`)، للأوامر المساعدة (على سبيل المثال `iroha_cli app sorafs fetch`) يمكن أن تقوم بالتوصيل المرحلة تتجاوز سياسة عدم الكشف عن هويته الافتراضية.

## الأتمتة

يعمل هذا المساعد `cargo xtask` تلقائيًا على إنشاء الصور والتقاط القطع الأثرية.

1. **تعديل الجدول الزمني الإقليمي**

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

   تحدد المدة الملحقات `s` أو `m` أو `h` أو `d`. أمر الشراء `artifacts/soranet_pq_rollout_plan.json` وملخص تخفيض السعر (`artifacts/soranet_pq_rollout_plan.md`)، الذي يمكن تطبيقه عند تغيير الطلب.

2. **التقاط قطع أثرية من الحفر شاركها**

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

   قم بنسخ الملفات المحذوفة في `artifacts/soranet_pq_rollout/<timestamp>_<label>/`، واحصل على هضم BLAKE3 لكل قطعة أثرية واكتب `rollout_capture.json` باستخدام بيانات التعريف وEd25519 قم بنشر الحمولة القصوى. استخدم هذا المفتاح الخاص الذي يتضمن دقائق التدريب على الحرائق التي يمكن من خلالها التحقق من صحة الالتقاط.

## ماتريشا فلاغوف SDK & CLI| السطح | الكناري (المرحلة أ) | المنحدر (المرحلة ب) | الافتراضي (المرحلة ج) |
|---------|------------------|----------------|-------------------|
| جلب `sorafs_cli` | `--anonymity-policy stage-a` أو تشغيل المرحلة | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| التكوين المنسق JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| تكوين عميل الصدأ (`iroha.toml`) | `rollout_phase = "canary"` (افتراضي) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| `iroha_cli` الأوامر الموقعة | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| جافا/أندرويد `GatewayFetchOptions` | `setRolloutPhase("canary")`، اختياري `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")`، اختياري `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")`، اختياري `.ANON_STRICT_PQ` |
| مساعدين منسق جافا سكريبت | `rolloutPhase: "canary"` أو `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| بايثون `fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
| سويفت `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

تقوم جميع أدوات SDK بتبديل الخرائط إلى محلل المسرح والمنسق (`crates/sorafs_orchestrator/src/lib.rs:365`)، ويتم إجراء عمليات النشر متعددة اللغات في خطوة القفل مع المرحلة الأساسية.

## قائمة مراجعة جدولة الكناري

1. **التجربة المبدئية (T ناقص أسبوعين)**- تأكد من أن معدل انقطاع التيار الكهربائي للمرحلة A =70% للمنطقة (`sorafs_orchestrator_pq_candidate_ratio`).
   - قم بتخطيط فتحة مراجعة الإدارة التي يتم فتحها بواسطة الكناري.
   - قم بتثبيت `sorafs.gateway.rollout_phase = "ramp"` في التدريج (إزالة المنسق JSON وإعادة النشر) واستخدام خط أنابيب الترويج للتشغيل الجاف.

2. **تتابع الكناري (يوم T)**

   - التطوير في منطقة معينة، من خلال `rollout_phase = "ramp"` للمنسق وإظهار المرحلات السعيدة.
   - راجع "أحداث السياسة لكل نتيجة" و"معدل انقطاع التيار الكهربائي" في لوحة معلومات PQ Ratchet (مع لوحة الطرح) في ذاكرة التخزين المؤقت TTL للحماية المتطورة.
   - قم بتصوير اللقطات `sorafs_cli guard-directory fetch` إلى مخزن التدقيق وبعده.

3. ** كناري العميل/SDK (T بالإضافة إلى أسبوع واحد)**

   - قم بإلغاء تحديد `rollout_phase = "ramp"` في تكوينات العميل أو قم بإلغاء تجاوز `stage-b` لجميع مجموعات SDK.
   - قم بتسجيل فروق القياس عن بعد (`sorafs_orchestrator_policy_events_total`، المجمعة على `client_id` و`region`) واستخدمها في سجل حوادث التشغيل.

4. **العرض الترويجي الافتراضي (T بالإضافة إلى 3 أسابيع)**

   - بعد حوكمة تسجيل الخروج، قم بإلغاء تكوينات المنسق والعميل لـ `rollout_phase = "default"` وقم بإعداد قائمة التحقق من جاهزية النشر في عناصر الإصدار.

## قائمة مراجعة الحوكمة والأدلة| تغير المرحلة | بوابة الترويج | حزمة الأدلة | لوحات المعلومات والتنبيهات |
|--------------|----------------|---------------------------------|-----|
| كناري -> المنحدر *(معاينة المرحلة ب)* | معدل انقطاع المرحلة A = 0.7 عند الترويج للمنطقة، التحقق من تذكرة Argon2 p95  الافتراضي *(فرض المرحلة C)* | 30 يومًا SN16 احتراق القياس عن بعد، `sn16_handshake_downgrade_total` مسطح على خط الأساس، `sorafs_orchestrator_brownouts_total` فقط في وقت العميل الكناري، ويتم مسح بروفة الوكيل. | نسخة `sorafs_cli proxy set-mode --mode gateway|direct`، вывод `promtool test rules dashboards/alerts/soranet_handshake_rules.yml`، سجل `sorafs_cli guard-directory verify`، حزمة مدمجة `cargo xtask soranet-rollout-capture --label default`. | بالإضافة إلى لوحة PQ Ratchet بالإضافة إلى لوحات الرجوع إلى إصدار أقدم SN16، المتوفرة في `docs/source/sorafs_orchestrator_rollout.md` و`dashboards/grafana/soranet_privacy_metrics.json`. || خفض رتبة الطوارئ / الاستعداد للتراجع | يتم تشغيله عند استكمال عدادات الرجوع إلى إصدار أقدم، أو التحقق من التحقق من دليل الحماية أو أحداث الرجوع إلى إصدار أقدم في المخزن المؤقت `/policy/proxy-toggle`. | قائمة التحقق من `docs/source/ops/soranet_transport_rollback.md` والسجل `sorafs_cli guard-directory import` / `guard-cache prune` و`cargo xtask soranet-rollout-capture --label rollback` وتذاكر الحوادث وقوالب الإشعارات. | `dashboards/grafana/soranet_pq_ratchet.json`، `dashboards/grafana/soranet_privacy_metrics.json` وحزم التنبيه (`dashboards/alerts/soranet_handshake_rules.yml`، `dashboards/alerts/soranet_privacy_rules.yml`). |

- تم العثور على قطعة أثرية في `artifacts/soranet_pq_rollout/<timestamp>_<label>/` مع `rollout_capture.json` التي تتضمن حزم الإدارة لوحة النتائج وتتبعات promtool والهضم.
- استخدم ملخص SHA256 للتوثيق المحمي (دقائق PDF، حزمة الالتقاط، لقطات الحراسة) إلى محاضر الترويج، بحيث يمكن الحصول على موافقات البرلمان دون الوصول إلى مجموعة التدريج.
- قم بالاشتراك في خطة القياس عن بعد في تذكرة الترويج للتأكد من أن `docs/source/soranet/snnet16_telemetry_plan.md` يتمتع بميزة أساسية لتخفيض المفردات وعتبات التنبيه.

## تحديثات لوحة المعلومات والقياس عن بعد

`dashboards/grafana/soranet_pq_ratchet.json` قم الآن بمراجعة لوحة التعليقات التوضيحية "خطة الطرح"، والتي تنضم إلى دليل التشغيل هذا وتستعرض المرحلة التالية التي يمكن من خلالها مراجعة مراجعات الإدارة المرحلة النشطة. اتبع لوحة الوصف المتزامنة مع مقابض التحسين.

لتنبيه المستخدمين إلى أن القواعد الفعلية تستخدم التصنيف `stage`، لأن المراحل الافتراضية والكناري تؤدي إلى تفعيل عتبات السياسة المتجاوزة (`dashboards/alerts/soranet_handshake_rules.yml`).## خطافات التراجع

### الافتراضي -> المنحدر (المرحلة ج -> المرحلة ب)

1. قم بتخفيض ترتيب المنسق من خلال `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` (وتابعه في تكوينات SDK)، لتتم إعادة المرحلة B إلى جميع الأسطول.
2. قم بإدخال العملاء في ملف تعريف النقل الآمن من خلال `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"`، مع الاحتفاظ بنص لسير عمل إمكانية التدقيق `/policy/proxy-toggle`.
3. قم بتثبيت `cargo xtask soranet-rollout-capture --label rollback-default` من خلال اختلافات دليل الحماية ومخرجات promtool ولقطات شاشة لوحة المعلومات تحت `artifacts/soranet_pq_rollout/`.

### المنحدر -> الكناري (المرحلة ب -> المرحلة أ)

1. قم باستيراد لقطة دليل الحراسة، قبل الترقية، من خلال `sorafs_cli guard-directory import --guard-directory guards.json` وقم بإعادة التثبيت مرة أخرى إلى `sorafs_cli guard-directory verify`، لتتضمن حزمة التخفيض التجزئة.
2. تثبيت `rollout_phase = "canary"` (أو تجاوز `anonymity_policy stage-a`) في إعدادات المنسق والعميل، ثم قم بإظهار مثقاب السقاطة PQ من [PQratchet runbook](./pq-ratchet-runbook.md)، لذلك доказать خط أنابيب الرجوع إلى إصدار أقدم.
3. قم بتنزيل لقطات شاشة القياس عن بعد PQ Ratchet وSN16 بالإضافة إلى نتائج التنبيه لسجل الحوادث قبل إدارة الحكم.

### تذكير الدرابزين

- قم بالرجوع إلى `docs/source/ops/soranet_transport_rollback.md` عند خفض الرتبة وإصلاح وقت التخفيف مثل `TODO:` في أداة تعقب الطرح للأعمال التالية.
- قم بالرجوع إلى `dashboards/alerts/soranet_handshake_rules.yml` و`dashboards/alerts/soranet_privacy_rules.yml` من خلال إعادة إنشاء `promtool test rules` وما بعده، حيث سيتم دعم تنبيه الانجراف من خلال حزمة الالتقاط.