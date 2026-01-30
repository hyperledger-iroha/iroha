---
lang: he
direction: rtl
source: docs/portal/docs/ministry/ai-moderation-runner.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
lang: ar
direction: rtl
source: docs/portal/docs/ministry/ai-moderation-runner.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 00cf1d37cf06d24b6eb7b2acba6b5c2ec3c3fae249b5cb6055384ca19ceaefac
source_last_modified: "2025-11-10T16:27:31.384538+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: مواصفة مشغّل الإشراف بالذكاء الاصطناعي
summary: تصميم لجنة إشراف حتمي لتسليم وزارة المعلومات (MINFO-1).
---

# مواصفة مشغّل الإشراف بالذكاء الاصطناعي

تفي هذه المواصفة بجانب التوثيق من **MINFO-1 — Establish AI moderation baseline**. وهي تحدد عقد التنفيذ الحتمي لخدمة الإشراف التابعة لوزارة المعلومات بحيث تتمكن كل بوابة من تشغيل مسارات عمل متطابقة قبل مسارات الاستئناف والشفافية (SFM-4/SFM-4b). كل السلوك الموصوف هنا معياري ما لم يُذكر صراحةً أنه معلوماتي.

## 1. الأهداف والنطاق
- توفير لجنة إشراف قابلة للإعادة تقيم محتوى البوابة (العناصر، manifests، البيانات الوصفية، الصوت) باستخدام نماذج غير متجانسة.
- ضمان تنفيذ حتمي عبر المشغلين: opset ثابت، تقطيع رمزي ببذور، دقة محدودة، وآثار (artefacts) ذات نسخ.
- إنتاج آثار جاهزة للتدقيق: manifests، scorecards، أدلة معايرة، وملخصات شفافية مناسبة للنشر في DAG الحوكمة.
- إتاحة القياس عن بُعد بحيث يمكن لـ SRE اكتشاف الانحراف والإيجابيات الكاذبة والتوقف دون جمع بيانات المستخدم الخام.

## 2. عقد التنفيذ الحتمي
- **Runtime:** ONNX Runtime 1.19.x (CPU backend) مُجمّع مع تعطيل AVX2 وخيار `--enable-extended-minimal-build` للإبقاء على مجموعة opcodes ثابتة. تشغيلات CUDA/Metal محظورة صراحةً في الإنتاج.
- **Opset:** `opset=17`. النماذج التي تستهدف opsets أحدث يجب خفضها والتحقق منها قبل الاعتماد.
- **اشتقاق البذرة:** كل تقييم يستخرج بذرة RNG من `BLAKE3(content_digest || manifest_id || run_nonce)` حيث تأتي `run_nonce` من manifest المعتمد حوكميًا. تُغذي البذور كل المكوّنات العشوائية (beam search، تبديلات dropout) ليكون الناتج قابلاً لإعادة الإنتاج بتطابق البتات.
- **التحزيء:** عامل واحد لكل نموذج. ينسّق مشغّل runner التزامن لتجنب سباقات الحالة المشتركة. تعمل مكتبات BLAS بوضع أحادي الخيط.
- **الأعداد:** تراكم FP16 ممنوع. استخدم وسائط FP32 وقيّد المخرجات إلى أربع منازل عشرية قبل التجميع.

## 3. تكوين اللجنة
تحتوي اللجنة الأساسية على ثلاث عائلات من النماذج. يمكن للحوكمة إضافة نماذج، لكن يجب أن يبقى الحد الأدنى للنصاب مستوفى.

| العائلة | النموذج الأساسي | الغرض |
|--------|----------------|---------|
| الرؤية | OpenCLIP ViT-H/14 (ضبط أمان) | يكتشف تهريبًا بصريًا، العنف، مؤشرات CSAM. |
| متعدد الوسائط | LLaVA-1.6 34B Safety | يلتقط تفاعلات النص + الصورة، القرائن السياقية، والتحرش. |
| إدراكي | تجميعة pHash + aHash + NeuralHash-lite | كشف سريع للتطابقات شبه المكررة واستدعاء المواد الضارة المعروفة. |

كل مدخل نموذج يحدد:
- `model_id` (UUID)
- `artifact_digest` (BLAKE3-256 لصورة OCI)
- `weights_digest` (BLAKE3-256 لملف ONNX أو blob safetensors المدمج)
- `opset` (يجب أن يساوي `17`)
- `weight` (وزن اللجنة، الافتراضي `1.0`)
- `critical_labels` (مجموعة تسميات تُطلق `Escalate` فورًا)
- `max_eval_ms` (حدود حماية لمراقبات حتمية)

## 4. manifests ونتائج Norito

### 4.1 manifest اللجنة
```norito
struct AiModerationManifestV1 {
    manifest_id: Uuid,
    issued_at: Timestamp,
    runner_hash: Digest32,
    runtime_version: String,
    models: Vec<AiModerationModelV1>,
    calibration_dataset: DatasetReferenceV1,
    calibration_hash: Digest32,
    thresholds: AiModerationThresholdsV1,
    run_nonce: Digest32,
    governance_signature: Signature,
}

struct AiModerationModelV1 {
    model_id: Uuid,
    family: AiModerationFamilyV1, // vision | multimodal | perceptual | audio
    artifact_digest: Digest32,
    weights_digest: Digest32,
    opset: u8,
    weight: f32,
    critical_labels: Vec<String>,
    max_eval_ms: u32,
}
```

### 4.2 نتيجة التقييم
```norito
struct AiModerationResultV1 {
    manifest_id: Uuid,
    request_id: Uuid,
    content_digest: Digest32,
    content_uri: String,
    content_class: ModerationContentClassV1, // manifest | chunk | metadata | audio
    model_scores: Vec<AiModerationModelScoreV1>,
    combined_score: f32,
    verdict: ModerationVerdictV1, // pass | quarantine | escalate
    executed_at: Timestamp,
    execution_ms: u32,
    runner_hash: Digest32,
    annotations: Option<Vec<String>>,
}

struct AiModerationModelScoreV1 {
    model_id: Uuid,
    score: f32,
    threshold: f32,
    confidence: f32,
    label: Option<String>,
}
```

يجب على runner إصدار `AiModerationDigestV1` حتمي (BLAKE3 على النتيجة المُسلسلة) لسجلات الشفافية وإلحاق النتائج بدفتر الإشراف عندما لا يكون الحكم `pass`.

### 4.3 manifest لمجموعة الخصوم

يدخل مشغلو البوابات الآن manifestًا مرافقًا يعدد “عائلات” التجزئة/التمثيلات الإدراكية المشتقة من تشغيلات المعايرة:

```norito
struct AdversarialCorpusManifestV1 {
    schema_version: u16,                // must equal 1
    issued_at_unix: u64,
    cohort_label: Option<String>,       // e.g. "2026-Q1"
    families: Vec<AdversarialPerceptualFamilyV1>,
}

struct AdversarialPerceptualFamilyV1 {
    family_id: Uuid,
    description: String,
    variants: Vec<AdversarialPerceptualVariantV1>,
}

struct AdversarialPerceptualVariantV1 {
    variant_id: Uuid,
    attack_vector: String,
    reference_cid_b64: Option<String>,
    perceptual_hash: Option<Digest32>,   // Goldilocks hash, BLAKE3 domain separated
    hamming_radius: u8,                  // ≤ 32
    embedding_digest: Option<Digest32>,  // BLAKE3 of quantised embedding vector
    notes: Option<String>,
}
```

تعيش البنية في `crates/iroha_data_model/src/sorafs/moderation.rs` ويتم التحقق منها عبر `AdversarialCorpusManifestV1::validate()`. يتيح هذا manifest لمحمل denylist في البوابة ملء إدخالات `perceptual_family` التي تحظر عناقيد كاملة من شبه المكررات بدلًا من بايتات فردية. يوضح fixture قابل للتشغيل (`docs/examples/ai_moderation_perceptual_registry_202602.json`) البنية المتوقعة ويغذي denylist المثال مباشرة.

## 5. خط تنفيذ العمل
1. تحميل `AiModerationManifestV1` من DAG الحوكمة. ارفض إذا لم تتطابق `runner_hash` أو `runtime_version` مع الثنائي المنشور.
2. جلب آثار النماذج عبر OCI digest مع التحقق من digests قبل التحميل.
3. بناء دفعات تقييم حسب نوع المحتوى؛ يجب ترتيبها وفق `(content_digest, manifest_id)` لضمان التجميع الحتمي.
4. تنفيذ كل نموذج باستخدام البذرة المشتقة. بالنسبة للتجزئات الإدراكية، اجمع التجميعة عبر تصويت الأغلبية → score ضمن `[0,1]`.
5. تجميع الدرجات في `combined_score` باستخدام نسبة مقصوصة موزونة:
   ```
   combined = Σ_i weight_i * clamp(score_i / threshold_i, 0, 1) / Σ_i weight_i
   ```
6. إنتاج `ModerationVerdictV1`:
   - `escalate` إذا أطلقت أي `critical_labels` أو إذا `combined ≥ thresholds.escalate`.
   - `quarantine` إذا كان أعلى من `thresholds.quarantine` وأقل من `escalate`.
   - `pass` خلاف ذلك.
7. حفظ `AiModerationResultV1` ووضع العمليات اللاحقة في الطابور:
   - خدمة الحجر (إذا تصاعد الحكم/الحجر)
   - كاتب سجل الشفافية (`ModerationLedgerV1`)
   - مُصدّر القياس عن بُعد

## 6. المعايرة والتقييم
- **مجموعات البيانات:** تستخدم المعايرة الأساسية corpus مختلطًا بموافقة فريق السياسات. يُسجل المرجع في `calibration_dataset`.
- **المقاييس:** احسب Brier score وExpected Calibration Error (ECE) وAUROC لكل نموذج والحكم المجمّع. يجب أن تحافظ المعايرة الشهرية على `Brier ≤ 0.18` و`ECE ≤ 0.05`. تُخزّن النتائج في شجرة تقارير SoraFS (مثل [معايرة فبراير 2026](../sorafs/reports/ai-moderation-calibration-202602.md)).
- **الجدول:** معايرة شهرية (أول اثنين). يُسمح بالمعايرة الطارئة إذا اشتعلت تنبيهات الانحراف.
- **العملية:** شغّل خط التقييم الحتمي على مجموعة المعايرة، أعد توليد `thresholds`، حدّث manifest، وجهّز التغييرات لتصويت الحوكمة.

## 7. التغليف والنشر
- بناء صور OCI عبر `docker buildx bake -f docker/ai_moderation.hcl`.
- تتضمن الصور:
  - بيئة Python مُثبتة (`poetry.lock`) أو ثنائي Rust `Cargo.lock`.
  - مجلد `models/` بأوزان ONNX ممهورة بالهاش.
  - نقطة دخول `run_moderation.py` (أو مكافئ Rust) تُعرّف API HTTP/gRPC.
- نشر الآثار إلى `registry.sora.net/ministry/ai-moderation/<model>@sha256:<digest>`.
- يُشحن ثنائي runner كجزء من crate `sorafs_ai_runner`. يضمن خط البناء تضمين hash للmanifest داخل الثنائي (مكشوف عبر `/v1/info`).

## 8. القياس عن بُعد والملاحظة
- مقاييس Prometheus:
  - `moderation_requests_total{verdict}`
  - `moderation_model_score_bucket{model_id,label}`
  - `moderation_combined_score_bucket`
  - `moderation_inference_latency_seconds_bucket`
  - `moderation_runner_manifest_info{manifest_id, runtime_version}`
- السجلات: أسطر JSON تحتوي `request_id` و`manifest_id` و`verdict` وdigest للنتيجة المخزنة. تُخفى الدرجات الخام إلى منزلتين عشريتين في السجلات.
- لوحات المعلومات محفوظة في `dashboards/grafana/ministry_moderation_overview.json` (تُنشر مع أول تقرير معايرة).
- عتبات التنبيه:
  - توقف الاستقبال (`moderation_requests_total` متوقف لمدة 10 دقائق).
  - كشف الانحراف (تغير متوسط درجة النموذج >20% مقارنة بمتوسط 7 أيام).
  - تراكم الإيجابيات الكاذبة (طابور الحجر > 50 عنصرًا لأكثر من 30 دقيقة).

## 9. الحوكمة والتحكم بالتغييرات
- تتطلب manifests توقيعين: عضو مجلس الوزارة + قائد SRE للموديريشن. تُسجّل التواقيع في `AiModerationManifestV1.governance_signature`.
- تتبع التغييرات `ModerationManifestChangeProposalV1` عبر Torii. تُسجّل الـ hashes في DAG الحوكمة؛ يُحظر النشر حتى إقرار المقترح.
- تتضمن ثنائيات runner قيمة `runner_hash`؛ ترفض CI النشر إذا اختلفت الـ hashes.
- الشفافية: `ModerationScorecardV1` أسبوعي يلخص الحجم ومزيج الأحكام ونتائج الاستئناف. يُنشر في بوابة برلمان Sora.

## 10. الأمان والخصوصية
- تستخدم digests المحتوى BLAKE3. لا تُحفظ الحمولة الخام خارج الحجر.
- الوصول إلى الحجر يتطلب موافقات Just-In-Time؛ تُسجل جميع عمليات الوصول.
- يعزل runner المحتوى غير الموثوق بحد ذاكرة 512 MiB وحواجز زمنية 120 ثانية.
- الخصوصية التفاضلية غير مطبقة هنا؛ تعتمد البوابات على الحجر ومسارات التدقيق. تتبع سياسات التنقيح خطة الامتثال للبوابة (`docs/source/sorafs_gateway_compliance_plan.md`؛ نسخة البوابة قيد الانتظار).

## 11. نشر المعايرة (2026-02)
- **Manifest:** `docs/examples/ai_moderation_calibration_manifest_202602.json`
  يسجل `AiModerationManifestV1` الموقّع حوكميًا (ID
  `c9bdf0b2-63a3-4a90-8d70-908d119c2c7e`)، مرجع مجموعة البيانات
  `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`، hash للrunner
  `ea3c0fd0ff4bd4510e94c7c293b261f601cc0c4f9fbacd99b0401d233a7cdc20`، وعتبات معايرة 2026-02 (`quarantine = 0.42`، `escalate = 0.78`).
- **Scoreboard:** `docs/examples/ai_moderation_calibration_scorecard_202602.json`
  إضافة إلى التقرير القابل للقراءة
  `[SoraFS Reports › AI Moderation Calibration 2026-02](../sorafs/reports/ai-moderation-calibration-202602.md)`
  توثق Brier وECE وAUROC ومزيج الأحكام لكل نموذج. حققت المقاييس المجمعة الأهداف (`Brier = 0.126`، `ECE = 0.034`).
- **لوحات المعلومات والتنبيهات:** `dashboards/grafana/ministry_moderation_overview.json`
  و`dashboards/alerts/ministry_moderation_rules.yml` (مع اختبارات الانحدار في
  `dashboards/alerts/tests/ministry_moderation_rules.test.yml`) توفر مراقبة الاستقبال/الكمون/الانحراف المطلوبة للإطلاق.

## 12. مخطط قابلية إعادة الإنتاج والمُتحقق (MINFO-1b)
- تعيش أنواع Norito القياسية الآن بجانب بقية مخطط SoraFS في
  `crates/iroha_data_model/src/sorafs/moderation.rs`. تلتقط البنى
  `ModerationReproManifestV1`/`ModerationReproBodyV1` UUID للmanifest، hash للrunner، digests للنماذج، مجموعة العتبات، ومادة البذور.
  يفرض `ModerationReproManifestV1::validate` نسخة المخطط
  (`MODERATION_REPRO_MANIFEST_VERSION_V1`)، ويضمن أن كل manifest يحتوي على نموذج واحد على الأقل ومُوقّع، ويتحقق من كل `SignatureOf<ModerationReproBodyV1>` قبل إعادة ملخص قابل للقراءة آليًا.
- يمكن للمشغلين استدعاء المُتحقق المشترك عبر
  `sorafs_cli moderation validate-repro --manifest=PATH [--format=json|norito]`
  (منفذ في `crates/sorafs_orchestrator/src/bin/sorafs_cli.rs`). يقبل الـ CLI
  إما الآثار JSON المنشورة في
  `docs/examples/ai_moderation_calibration_manifest_202602.json` أو ترميز Norito الخام ويطبع عدد النماذج/التواقيع والطابع الزمني للmanifest بعد نجاح التحقق.
- تستخدم البوابات والأتمتة المساعد نفسه لرفض manifests قابلية إعادة الإنتاج بشكل حتمي عندما ينحرف المخطط أو تنقص digests أو تفشل التواقيع.
- تتبع حزم corpus العدائي النمط نفسه:
  `sorafs_cli moderation validate-corpus --manifest=PATH [--format=json|norito]`
  يحلل `AdversarialCorpusManifestV1`، يفرض نسخة المخطط، ويرفض manifests التي تفتقد العائلات أو المتغيرات أو بيانات البصمة. تُصدر عمليات النجاح الطابع الزمني للإصدار ووسم الدفعة وعدد العائلات/المتغيرات لتمكين المشغلين من تثبيت الدليل قبل تحديث إدخالات denylist الموصوفة في القسم 4.3.

## 13. المتابعات المفتوحة
- تستمر نوافذ المعايرة الشهرية بعد 2026-03-02 باتباع إجراء القسم 6؛ انشر `ai-moderation-calibration-<YYYYMM>.md` مع حزم manifest/scorecard المحدثة ضمن شجرة تقارير SoraFS.
- ما زال MINFO-1b وMINFO-1c (متحققو manifests لإعادة الإنتاج وسجل corpus العدائي) قيد التتبع بشكل منفصل في roadmap.
