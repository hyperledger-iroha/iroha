---
lang: ar
direction: rtl
source: docs/portal/docs/soranet/pq-rollout-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: خطة الطرح PQ
العنوان: Playbook de rollout pos-quantico SNNet-16G
Sidebar_label: مخطط الطرح PQ
الوصف: دليل تشغيلي لتعزيز المصافحة الهجينة X25519+ML-KEM do SoraNet الكناري للمرحلات الافتراضية والعملاء وSDK.
---

:::ملاحظة فونتي كانونيكا
هذه الصفحة تحمل عنوان `docs/source/soranet/pq_rollout_plan.md`. Mantenha ambas as copias sincronzadas.
:::

يختتم SNNet-16G عملية الطرح الكمي لنقل SoraNet. تسمح المقابض `rollout_phase` للمشغلين بتنسيق تعزيز حتمي لمتطلبات الحماية الفعلية للمرحلة A لتغطية الأغلبية المرحلة B ووضعية PQ المفتوحة للمرحلة C عن طريق تحرير JSON/TOML بالكامل لكل سطح.

Este قواعد اللعب كوبر:

- تعريفات المسار ومقابض التكوين الجديدة (`sorafs.gateway.rollout_phase`، `sorafs.rollout_phase`) بدون قاعدة تعليمات برمجية (`crates/iroha_config/src/parameters/actual.rs:2230`، `crates/iroha/src/config/user.rs:251`).
- خريطة علامات SDK وCLI ليرافق كل عميل عملية الطرح.
- توقعات الجدولة الخاصة بمرحل الكناري/العميل ولوحات المعلومات الخاصة بالإدارة التي ستروج لها (`dashboards/grafana/soranet_pq_ratchet.json`).
- خطافات التراجع والمراجع لدليل التدريب على الحرائق ([PQ دليل السقاطة](./pq-ratchet-runbook.md)).

## مابا دي فاسيز| `rollout_phase` | مرحلة المجهول الفعال | افييتو بادراو | استخدام تيبيكو |
|-----------------|-------------------------------------------|----------------|---------------|
| `canary` | `anon-guard-pq` (المرحلة أ) | قم بالبدء في أقل من حماية PQ من خلال الدائرة أثناء الخروج من الماء. | خط الأساس والأسبوعين الأوليين لطائر الكناري. |
| `ramp` | `anon-majority-pq` (المرحلة ب) | شاهد التحديد للمرحلات PQ com >= دويس تيركوس دي التغطية؛ المرحلات الكلاسيكية ficam como الاحتياطية. | كناري من أجل المرحلات؛ تبديل المعاينة في SDK. |
| `default` | `anon-strict-pq` (المرحلة ج) | دوائر السيارة تحتوي على PQ وتحمل إنذارات خفض المستوى. | الترويج النهائي للقياس عن بعد والتوقيع على الإدارة. |

إذا كان السطح محددًا بشكل صريح `anonymity_policy`، فسيتم تحديده على الفور لهذا المكون. قم بحذف المرحلة الصريحة الآن لتأجيل قيمة `rollout_phase` حتى يتمكن المشغلون من العمل في بيئة ما وإبعاد العملاء عنهم.

## مرجع التكوين

### المنسق (`sorafs_gateway`)

```toml
[sorafs.gateway]
# Promote to Stage B (majority-PQ) canary
rollout_phase = "ramp"
# Optional: force a specific stage independent of the phase
# anonymity_policy = "anon-majority-pq"
```

يقوم المُحمل بتحليل مرحلة التراجع في وقت التشغيل (`crates/sorafs_orchestrator/src/lib.rs:2229`) والعرض عبر `sorafs_orchestrator_policy_events_total` و`sorafs_orchestrator_pq_ratio_*`. Veja `docs/examples/sorafs_rollout_stage_b.toml` و`docs/examples/sorafs_rollout_stage_c.toml` لتطبيق المقتطفات السريعة.

### عميل الصدأ / `iroha_cli`

```toml
[sorafs]
# Keep clients aligned with orchestrator promotion cadence
rollout_phase = "default"
# anonymity_policy = "anon-strict-pq"  # optional explicit override
```تم تسجيل `iroha::Client` الآن للتحليل الفوري (`crates/iroha/src/client.rs:2315`) حتى تتمكن الأوامر المساعدة (على سبيل المثال `iroha_cli app sorafs fetch`) من الإبلاغ عن الحالة الفعلية جنبًا إلى جنب مع سياسة مجهولة المصدر.

## أوتوماكو

هل يساعد هذا `cargo xtask` في أتمتة عملية الجدولة والتقاط العناصر.

1. ** جدول زمني إقليمي **

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

   الفترات التالية هي `s`، `m`، `h` أو `d`. قم بإصدار الأمر `artifacts/soranet_pq_rollout_plan.json` واستئناف Markdown (`artifacts/soranet_pq_rollout_plan.md`) الذي يمكن إرساله مع طلب التغيير.

2. **التقاط القطع الأثرية بالحفر مع القتلة**

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

   لأمر بنسخ الملفات المقدمة لـ `artifacts/soranet_pq_rollout/<timestamp>_<label>/`، احسب BLAKE3 لكل قطعة مصطنعة واكشف `rollout_capture.json` لمنافسة البيانات الوصفية ودمج Ed25519 حول الحمولة. استخدم مفتاحًا خاصًا يمكنك تجربته خلال دقائق من التدريب على تفعيل الحوكمة بسرعة.

## Matriz de flags SDK & CLI| السطح | الكناري (المرحلة أ) | المنحدر (المرحلة ب) | الافتراضي (المرحلة ج) |
|---------|------------------|----------------|-------------------|
| جلب `sorafs_cli` | `--anonymity-policy stage-a` أو قم بالموافقة عليه | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| التكوين المنسق JSON (`sorafs.gateway.rollout_phase`) | `canary` | `ramp` | `default` |
| تكوين عميل الصدأ (`iroha.toml`) | `rollout_phase = "canary"` (افتراضي) | `rollout_phase = "ramp"` | `rollout_phase = "default"` |
| `iroha_cli` الأوامر الموقعة | `--anonymity-policy stage-a` | `--anonymity-policy stage-b` | `--anonymity-policy stage-c` |
| جافا/أندرويد `GatewayFetchOptions` | `setRolloutPhase("canary")`، اختياري `setAnonymityPolicy(AnonymityPolicy.ANON_GUARD_PQ)` | `setRolloutPhase("ramp")`، اختياري `.ANON_MAJORIY_PQ` | `setRolloutPhase("default")`، اختياري `.ANON_STRICT_PQ` |
| مساعدين منسق جافا سكريبت | `rolloutPhase: "canary"` أو `anonymityPolicy: "anon-guard-pq"` | `"ramp"` / `"anon-majority-pq"` | `"default"` / `"anon-strict-pq"` |
| بايثون `fetch_manifest` | `rollout_phase="canary"` | `"ramp"` | `"default"` |
| سويفت `SorafsGatewayFetchOptions` | `anonymityPolicy: "anon-guard-pq"` | `"anon-majority-pq"` | `"anon-strict-pq"` |

يتم تعيين جميع مفاتيح تبديل SDK لمحلل المرحلة نفسها باستخدام المنسق (`crates/sorafs_orchestrator/src/lib.rs:365`)، بما في ذلك عمليات النشر متعددة اللغات من خلال خطوة القفل مع التكوين الفوري.

## قائمة التحقق من جدولة الكناري

1. **التجربة المبدئية (T ناقص أسبوعين)**- تأكد من أن معدل انقطاع التيار الكهربائي في المرحلة A أقل من 1% خلال الأسبوعين الأخيرين وأن تغطية PQ >=70% في المنطقة (`sorafs_orchestrator_pq_candidate_ratio`).
   - أجندة أو فتحة مراجعة الإدارة التي ستؤدي إلى إنشاء مجموعة الكناري.
   - تحديث `sorafs.gateway.rollout_phase = "ramp"` للتدريج (تحرير أو منسق JSON وإعادة النشر) وتشغيل خط الأنابيب الترويجي.

2. **تتابع الكناري (يوم T)**

   - ترقية منطقة من خلال تكوين `rollout_phase = "ramp"` بدون منسق وبيانات ترحيل المشاركين.
   - مراقبة "أحداث السياسة لكل نتيجة" و"معدل انقطاع التيار الكهربائي" بدون لوحة معلومات PQ Ratchet (من خلال لوحة الطرح) مرتين أو TTL لحماية ذاكرة التخزين المؤقت.
   - التقاط لقطات `sorafs_cli guard-directory fetch` قبل وبعد تخزين قاعة الاستماع.

3. ** كناري العميل/SDK (T بالإضافة إلى أسبوع واحد)**

   - Trocar for `rollout_phase = "ramp"` من خلال تكوينات العميل أو تجاوز `stage-b` لمجموعات SDK المخصصة.
   - التقاط اختلافات القياس عن بعد (`sorafs_orchestrator_policy_events_total` المجمعة بواسطة `client_id` و`region`) والإضافة إلى سجل حوادث الطرح.

4. **العرض الترويجي الافتراضي (T بالإضافة إلى 3 أسابيع)**

   - بعد تسجيل الخروج من الإدارة، قم بتغيير إعدادات المنسق والعميل لـ `rollout_phase = "default"` وقم بتدوير قائمة التحقق من الاستعداد التي تم حذفها لإصدار العناصر.

## قائمة مراجعة الحوكمة والأدلة| تغير المرحلة | بوابة الترويج | حزمة الأدلة | لوحات المعلومات والتنبيهات |
|--------------|----------------|---------------------------------|-----|
| كناري -> المنحدر *(معاينة المرحلة ب)* | معدل انقطاع التيار الكهربائي للمرحلة A = 0.7 في المنطقة المعززة، التحقق من تذكرة Argon2 p95  الافتراضي *(فرض المرحلة C)* | النسخ الاحتياطي للقياس عن بعد SN16 لمدة 30 يومًا نهائيًا، `sn16_handshake_downgrade_total` مسطح بدون خط أساسي، `sorafs_orchestrator_brownouts_total` صفر خلال فترة تسجيل العميل، وتجربة تبديل الوكيل. | النص `sorafs_cli proxy set-mode --mode gateway|direct`، إخراج `promtool test rules dashboards/alerts/soranet_handshake_rules.yml`، سجل `sorafs_cli guard-directory verify`، تم تدمير الحزمة `cargo xtask soranet-rollout-capture --label default`. | نفس اللوحة PQ Ratchet وألواح الرجوع إلى إصدار أقدم SN16 الموثقة في `docs/source/sorafs_orchestrator_rollout.md` و`dashboards/grafana/soranet_privacy_metrics.json`. || خفض رتبة الطوارئ / الاستعداد للتراجع | يتم تفعيلها عند عدادات الرجوع إلى إصدار أقدم، أو التحقق من دليل الحماية، أو تسجيل المخزن المؤقت `/policy/proxy-toggle` لأحداث الرجوع إلى إصدار أقدم باستمرار. | قائمة التحقق من `docs/source/ops/soranet_transport_rollback.md`، وسجلات `sorafs_cli guard-directory import` / `guard-cache prune`، و`cargo xtask soranet-rollout-capture --label rollback`، وتذاكر الأحداث، ونماذج الإشعارات. | `dashboards/grafana/soranet_pq_ratchet.json`، `dashboards/grafana/soranet_privacy_metrics.json`، وحزم التنبيه الشاملة (`dashboards/alerts/soranet_handshake_rules.yml`، `dashboards/alerts/soranet_privacy_rules.yml`). |

- أرمازين كل قطعة مصطنعة في `artifacts/soranet_pq_rollout/<timestamp>_<label>/` مع `rollout_capture.json` تعمل على أن تحتوي حزم الإدارة على لوحة النتائج وآثار الأدوات والخلاصات.
- يلخص الملحق SHA256 das evidencias carregadas (دقائق PDF، حزمة الالتقاط، لقطات الحراسة) كدقائق للترويج لكي يوافق البرلمان على إعادة إنتاجها دون الوصول إلى مجموعة التدريج.
- مرجع خطة القياس عن بعد لا يوجد تذكرة ترويجية لإثبات أن `docs/source/soranet/snnet16_telemetry_plan.md` سيتبع خطًا قانونيًا لمفردات التخفيض وعتبات التنبيه.

## تحديث لوحة القيادة والقياس عن بعد

يشتمل `dashboards/grafana/soranet_pq_ratchet.json` الآن على لوحة التعليقات التوضيحية "خطة الطرح" التي تربط قواعد اللعبة هذه وتظهر بشكل فوري حتى تؤكد مراجعات الإدارة مدى هذه المرحلة. استمر في وصف اللوحة المتزامنة مع مقابض التكوين المستقبلية.للتنبيه، تأكد من استخدام الأنظمة الموجودة أو التسمية `stage` بحيث يتم فصل عتبات السياسة بشكل افتراضي (`dashboards/alerts/soranet_handshake_rules.yml`).

## خطافات التراجع

### الافتراضي -> المنحدر (المرحلة ج -> المرحلة ب)

1. قم بخفض رتبة المنسق باستخدام `sorafs_cli config set --config orchestrator.json sorafs.gateway.rollout_phase ramp` (وتحدث عبر تكوينات SDK) حتى تتمكن المرحلة B من العودة إلى الوراء.
2. قم بإجبار العملاء على الوصول إلى ملف تعريف النقل الآمن عبر `sorafs_cli proxy set-mode --mode direct --note "sn16 rollback"`، والتقاط أو نسخ النص لمواصلة التدقيق في سير العمل `/policy/proxy-toggle`.
3. Rode `cargo xtask soranet-rollout-capture --label rollback-default` لفتح اختلافات دليل الحماية وإخراج promtool ولقطات شاشة لوحة القيادة في `artifacts/soranet_pq_rollout/`.

### المنحدر -> الكناري (المرحلة ب -> المرحلة أ)

1. قم باستيراد لقطة دليل الحماية التي تم التقاطها قبل الترويج باستخدام `sorafs_cli guard-directory import --guard-directory guards.json` واستخدم `sorafs_cli guard-directory verify` بشكل جديد حتى تتضمن حزمة التخفيض التجزئات.
2. اضبط `rollout_phase = "canary"` (أو تجاوز com `anonymity_policy stage-a`) في تكوينات المنسق والعميل، بعد أن تم استخدام مثقاب السقاطة PQ الجديد [PQ اسئلة Runbook](./pq-ratchet-runbook.md) لاختبار خط أنابيب الرجوع إلى إصدار أقدم.
3. تم تحديث لقطات الشاشة المرفقة من PQ Ratchet والقياس عن بعد SN16 بالإضافة إلى نتائج التنبيهات بسجل الأحداث قبل إخطار الإدارة.

### تذكير الدرابزين- المرجع `docs/source/ops/soranet_transport_rollback.md` يستمر في إجراء تخفيض الرتبة وتسجيل أي تخفيف مؤقت مثل العنصر `TODO:` بدون تعقب الطرح للمرافقة.
- قم بحفظ `dashboards/alerts/soranet_handshake_rules.yml` و`dashboards/alerts/soranet_privacy_rules.yml` من خلال تغطية `promtool test rules` قبل وبعد التراجع لتنبيه الانجراف الموثق جنبًا إلى جنب مع حزمة الالتقاط.