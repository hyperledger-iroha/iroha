---
lang: ja
direction: ltr
source: docs/portal/i18n/he/docusaurus-plugin-content-docs/current/ministry/ai-moderation-runner.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 34de0fc76ef9e84529b97974e7613f804cb45960abfead9ebedf0ac909d64878
source_last_modified: "2026-01-30T16:06:47+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: he
direction: rtl
source: docs/portal/docs/ministry/ai-moderation-runner.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
title: מפרט ראנר למודרציה מבוססת בינה מלאכותית
summary: עיצוב דטרמיניסטי של ועדת מודרציה למסירת משרד המידע (MINFO-1).
---

# מפרט ראנר למודרציה מבוססת בינה מלאכותית

המפרט הזה ממלא את רכיב התיעוד של **MINFO-1 — Establish AI moderation baseline**. הוא מגדיר את חוזה ההרצה הדטרמיניסטי של שירות המודרציה של משרד המידע, כך שכל gateway יוכל להריץ פייפליין זהה לפני זרימות הערעור והשקיפות (SFM-4/SFM-4b). כל ההתנהגות המתוארת כאן היא נורמטיבית אלא אם צוין במפורש שהיא אינפורמטיבית.

## 1. יעדים והיקף
- לספק ועדת מודרציה שניתנת לשחזור שמעריכה תוכן של gateway (אובייקטים, manifests, מטא־נתונים, אודיו) באמצעות מודלים הטרוגניים.
- להבטיח ביצוע דטרמיניסטי בין מפעילים: opset קבוע, טוקניזציה עם seed, דיוק מוגבל וארטיפקטים ממוספרים.
- להפיק ארטיפקטים מוכנים לביקורת: manifests, scorecards, ראיות כיול ו־digests לשקיפות המתאימים לפרסום ב־DAG הממשל.
- להציג טלמטריה כך ש־SRE יוכלו לזהות drift, חיוביות שגויות והשבתה בלי לאסוף נתוני משתמש גולמיים.

## 2. חוזה ביצוע דטרמיניסטי
- **Runtime:** ONNX Runtime 1.19.x (CPU backend) עם AVX2 כבוי ו־`--enable-extended-minimal-build` כדי לשמור על סט opcodes קבוע. רנטיימים של CUDA/Metal אסורים במפורש בפרודקשן.
- **Opset:** `opset=17`. מודלים שמכוונים ל־opset חדש יותר חייבים להיות מונמכים ומאומתים לפני קבלה.
- **גזירת seed:** כל הערכה גוזרת seed RNG מ־`BLAKE3(content_digest || manifest_id || run_nonce)` כאשר `run_nonce` מגיע מה־manifest שאושר בממשל. ה־seed מזין רכיבים סטוכסטיים (beam search, טוגלים של dropout) כך שהתוצאות יהיו משתחזרות ביט־לביט.
- **Threading:** worker אחד לכל מודל. האורקסטרטור של הראנר מתאם את המקביליות כדי למנוע race conditions במצב משותף. ספריות BLAS פועלות במצב חד־חוטי.
- **Numerics:** צבירה ב־FP16 אסורה. השתמשו בביניים FP32 והגבילו יציאות לארבע ספרות אחרי הנקודה לפני אגרגציה.

## 3. הרכב הוועדה
הוועדה הבסיסית כוללת שלוש משפחות מודלים. הממשל יכול להוסיף מודלים, אך המינימום של קוורום חייב להישמר.

| משפחה | מודל בסיס | מטרה |
|--------|----------------|---------|
| Vision | OpenCLIP ViT-H/14 (כוונון בטיחות) | מזהה הברחות ויזואליות, אלימות, אינדיקציות CSAM. |
| Multimodal | LLaVA-1.6 34B Safety | לוכד אינטראקציות טקסט + תמונה, רמזים קונטקסטואליים והטרדה. |
| Perceptual | אנסמבל pHash + aHash + NeuralHash-lite | זיהוי מהיר של כמעט־כפילויות ואחזור חומר מזיק מוכר. |

כל רשומת מודל מציינת:
- `model_id` (UUID)
- `artifact_digest` (BLAKE3-256 של תמונת OCI)
- `weights_digest` (BLAKE3-256 של ONNX או blob safetensors מאוחד)
- `opset` (חייב להיות `17`)
- `weight` (משקל הוועדה, ברירת מחדל `1.0`)
- `critical_labels` (קבוצת תוויות שמפעילות `Escalate` מיד)
- `max_eval_ms` (guardrail לווטצ׳דוגים דטרמיניסטיים)

## 4. manifests ותוצאות Norito

### 4.1 manifest הוועדה
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

### 4.2 תוצאת הערכה
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

הראנר חייב להפיק `AiModerationDigestV1` דטרמיניסטי (BLAKE3 על התוצאה המסודרת) עבור יומני שקיפות ולצרף תוצאות ללדג׳ר המודרציה כאשר הוורדיקט אינו `pass`.

### 4.3 manifest של קורפוס אדוורסרי

מפעילי gateway צורכים כעת manifest מלווה שמונה “משפחות” של hashes/embeddings תפיסתיים שמקורם בהרצות כיול:

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

הסכמה נמצאת ב־`crates/iroha_data_model/src/sorafs/moderation.rs` ומאומתת באמצעות `AdversarialCorpusManifestV1::validate()`. ה־manifest מאפשר לטעינת denylist של ה־gateway למלא ערכי `perceptual_family` שחוסמים אשכולות שלמים של כמעט־כפילויות במקום בייטים בודדים. fixture ניתן להרצה (`docs/examples/ai_moderation_perceptual_registry_202602.json`) מדגים את ה־layout הצפוי ומזין ישירות את ה־denylist לדוגמה.

## 5. פייפליין ביצוע
1. טען `AiModerationManifestV1` מ־DAG הממשל. דחה אם `runner_hash` או `runtime_version` אינם תואמים לבינארי המופעל.
2. משוך ארטיפקטים של מודלים באמצעות OCI digest, עם אימות digests לפני טעינה.
3. בנה באצ׳ים לפי סוג תוכן; הסדר חייב להיות `(content_digest, manifest_id)` כדי להבטיח אגרגציה דטרמיניסטית.
4. הרץ כל מודל עם ה־seed הנגזר. עבור hashes תפיסתיים, איחד את האנסמבל בהצבעת רוב → score ב־`[0,1]`.
5. אגרגציה ל־`combined_score` עם יחס מקוצץ משוקלל:
   ```
   combined = Σ_i weight_i * clamp(score_i / threshold_i, 0, 1) / Σ_i weight_i
   ```
6. הפק `ModerationVerdictV1`:
   - `escalate` אם אחת מ־`critical_labels` מופעלת או אם `combined ≥ thresholds.escalate`.
   - `quarantine` אם מעל `thresholds.quarantine` ומתחת ל־`escalate`.
   - `pass` אחרת.
7. התמד `AiModerationResultV1` ושגר תהליכים downstream:
   - שירות הסגר (אם הוורדיקט escalates/quarantines)
   - כותב יומן שקיפות (`ModerationLedgerV1`)
   - יצואן טלמטריה

## 6. כיול והערכה
- **Datasets:** כיול הבסיס משתמש בקורפוס מעורב באישור צוות מדיניות. ההפניה נרשמת ב־`calibration_dataset`.
- **Metrics:** חשב Brier, Expected Calibration Error (ECE) ו־AUROC לכל מודל ולוורדיקט המשולב. כיול חודשי חייב לשמור על `Brier ≤ 0.18` ו־`ECE ≤ 0.05`. התוצאות נשמרות בעץ הדוחות של SoraFS (למשל [כיול פברואר 2026](../sorafs/reports/ai-moderation-calibration-202602.md)).
- **Schedule:** כיול חודשי (יום שני הראשון). כיול חירום מותר אם מופעלות התראות drift.
- **Process:** הרץ את פייפליין ההערכה הדטרמיניסטי על סט הכיול, עדכן `thresholds`, עדכן את ה־manifest והכן לשלב ההצבעה בממשל.

## 7. אריזה ופריסה
- בנו תמונות OCI באמצעות `docker buildx bake -f docker/ai_moderation.hcl`.
- התמונות כוללות:
  - סביבת Python נעולה (`poetry.lock`) או בינארי Rust `Cargo.lock`.
  - תיקיית `models/` עם משקלי ONNX מאוחסנים כ־hash.
  - נקודת כניסה `run_moderation.py` (או מקביל Rust) שמחשפת API HTTP/gRPC.
- פרסמו ארטיפקטים ל־`registry.sora.net/ministry/ai-moderation/<model>@sha256:<digest>`.
- הבינארי של הראנר נשלח כחלק מ־crate `sorafs_ai_runner`. צינור הבנייה מטמיע את hash המניפסט בבינארי (נחשף דרך `/v1/info`).

## 8. טלמטריה ותצפיות
- מדדי Prometheus:
  - `moderation_requests_total{verdict}`
  - `moderation_model_score_bucket{model_id,label}`
  - `moderation_combined_score_bucket`
  - `moderation_inference_latency_seconds_bucket`
  - `moderation_runner_manifest_info{manifest_id, runtime_version}`
- לוגים: שורות JSON עם `request_id`, `manifest_id`, `verdict` ו־digest של התוצאה המאוחסנת. ציוני המקור מצומצמים לשתי ספרות אחרי הנקודה בלוגים.
- לוחות מחוונים נשמרים ב־`dashboards/grafana/ministry_moderation_overview.json` (מתפרסמים יחד עם דוח הכיול הראשון).
- ספי התראות:
  - חוסר קליטה (`moderation_requests_total` נעצר ל־10 דקות).
  - זיהוי drift (delta ממוצע של ציון מודל >20% ביחס לממוצע נע 7 ימים).
  - עומס חיוביות שגויות (תור הסגר > 50 פריטים למשך >30 דקות).

## 9. ממשל ובקרת שינויים
- manifests דורשים חתימה כפולה: חבר מועצת המשרד + מוביל SRE של מודרציה. החתימות נרשמות ב־`AiModerationManifestV1.governance_signature`.
- שינויים עוברים דרך `ModerationManifestChangeProposalV1` ב־Torii. ה־hashes נכנסים ל־DAG הממשל; הפריסה חסומה עד שההצעה מתקבלת.
- בינארי הראנר מטמיע `runner_hash`; ה־CI מסרב לפרוס אם ה־hashes שונים.
- שקיפות: `ModerationScorecardV1` שבועי שמסכם נפח, תמהיל וורדיקטים ותוצאות ערעור. מפורסם בפורטל פרלמנט סורה.

## 10. אבטחה ופרטיות
- דיג'סטים של תוכן משתמשים ב־BLAKE3. payloads גולמיים אינם נשמרים מחוץ להסגר.
- גישה להסגר דורשת אישורי Just‑In‑Time; כל הגישות מתועדות.
- הראנר מסנדבק תוכן לא אמין ומכיל מגבלת זיכרון 512 MiB ושומרי זמן של 120 שניות.
- פרטיות דיפרנציאלית אינה מיושמת כאן; ה־gateways נשענים על הסגר + תהליכי ביקורת. מדיניות ה־redaction עוקבת אחר תכנית התאימות של ה־gateway (`docs/source/sorafs_gateway_compliance_plan.md`; עותק פורטל ממתין).

## 11. פרסום כיול (2026-02)
- **Manifest:** `docs/examples/ai_moderation_calibration_manifest_202602.json`
  מתעד `AiModerationManifestV1` חתום ממשלית (ID
  `c9bdf0b2-63a3-4a90-8d70-908d119c2c7e`), הפניית dataset
  `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`, hash של הראנר
  `ea3c0fd0ff4bd4510e94c7c293b261f601cc0c4f9fbacd99b0401d233a7cdc20`, וספי כיול 2026-02 (`quarantine = 0.42`, `escalate = 0.78`).
- **Scoreboard:** `docs/examples/ai_moderation_calibration_scorecard_202602.json`
  יחד עם הדוח הקריא
  `[SoraFS Reports › AI Moderation Calibration 2026-02](../sorafs/reports/ai-moderation-calibration-202602.md)`
  מתעדים Brier, ECE, AUROC ותמהיל וורדיקטים לכל מודל. המדדים המשולבים עמדו ביעדים (`Brier = 0.126`, `ECE = 0.034`).
- **Dashboards & alerts:** `dashboards/grafana/ministry_moderation_overview.json`
  ו־`dashboards/alerts/ministry_moderation_rules.yml` (עם בדיקות רגרסיה ב־
  `dashboards/alerts/tests/ministry_moderation_rules.test.yml`) מספקים את סיפור המעקב הנדרש ל־ingest/latency/drift עבור ההשקה.

## 12. סכמת שחזור וולידטור (MINFO-1b)
- טיפוסי Norito קנוניים נמצאים כעת לצד סכמת SoraFS ב־
  `crates/iroha_data_model/src/sorafs/moderation.rs`. המבנים
  `ModerationReproManifestV1`/`ModerationReproBodyV1` כוללים UUID של ה־manifest, hash של הראנר, digests של מודלים, סט ספים וחומר seed.
  `ModerationReproManifestV1::validate` אוכף את גרסת הסכמה
  (`MODERATION_REPRO_MANIFEST_VERSION_V1`), מבטיח שלכל manifest יש לפחות מודל אחד ומחתים, ומאמת כל `SignatureOf<ModerationReproBodyV1>` לפני החזרת סיכום קריא־מכונה.
- מפעילים יכולים להריץ את הוולידטור המשותף דרך
  `sorafs_cli moderation validate-repro --manifest=PATH [--format=json|norito]`
  (ממומש ב־`crates/sorafs_orchestrator/src/bin/sorafs_cli.rs`). ה־CLI
  מקבל גם את ארטיפקטי ה־JSON שמפורסמים ב־
  `docs/examples/ai_moderation_calibration_manifest_202602.json` וגם את קידוד Norito הגולמי ומדפיס את כמות המודלים/חתימות ואת חותמת הזמן של ה־manifest לאחר אימות מוצלח.
- gateways ואוטומציה משתמשים באותו helper כדי לדחות manifests של שחזור בצורה דטרמיניסטית כאשר הסכמה נודדת, חסרים digests או אימות חתימות נכשל.
- חבילות corpus adversarial עוקבות אחר אותו דפוס:
  `sorafs_cli moderation validate-corpus --manifest=PATH [--format=json|norito]`
  מנתח את `AdversarialCorpusManifestV1`, אוכף את גרסת הסכמה ומסרב ל־manifests שחסרים משפחות, וריאנטים או מטא־דטה של טביעות. ריצות מוצלחות מפיקות חותמת זמן הנפקה, תווית cohort ומספרי משפחה/וריאנט כדי שהמפעילים יוכלו לקבע ראיות לפני עדכון רשומות ה־denylist של ה־gateway המתוארות בסעיף 4.3.

## 13. מעקבים פתוחים
- חלונות כיול חודשיים אחרי 2026-03-02 ממשיכים לפי ההליך בסעיף 6; יש לפרסם `ai-moderation-calibration-<YYYYMM>.md` יחד עם חבילות manifest/scorecard מעודכנות בעץ הדוחות של SoraFS.
- MINFO-1b ו־MINFO-1c (ולידטורים של manifests לשחזור ועוד רג’יסטרי corpus adversarial) נשארים במעקב בנפרד ב־roadmap.
