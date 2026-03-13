---
lang: am
direction: ltr
source: docs/portal/docs/ministry/ai-moderation-runner.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 00cf1d37cf06d24b6eb7b2acba6b5c2ec3c3fae249b5cb6055384ca19ceaefac
source_last_modified: "2025-12-29T18:16:35.119787+00:00"
translation_last_reviewed: 2026-02-07
title: AI Moderation Runner Specification
summary: Deterministic moderation committee design for the Ministry of Information (MINFO-1) deliverable.
translator: machine-google-reviewed
---

# AI አወያይ ሯጭ ዝርዝር

ይህ ዝርዝር የ **MINFO-1 - AI ማቋቋም የሰነድ ክፍልን ያሟላል።
ልከኝነት መነሻ**። እሱ የሚወስነውን የአፈፃፀም ውል ይገልጻል
የኢንፎርሜሽን ሚኒስቴር አወያይነት አገልግሎት እያንዳንዱ መግቢያ በር በተመሳሳይ መንገድ እንዲሠራ
ከይግባኝ በፊት የቧንቧ መስመሮች እና ግልጽነት ፍሰቶች (SFM-4/SFM-4b). ሁሉም ባህሪ
እዚህ ላይ የተገለጸው እንደ መረጃዊ በግልጽ ምልክት ካልተደረገበት በስተቀር መደበኛ ነው።

## 1. ግቦች እና ወሰን
- የመተላለፊያ ይዘትን የሚገመግም ሊባዛ የሚችል አወያይ ኮሚቴ ያቅርቡ
  የተለያዩ ሞዴሎችን በመጠቀም (ዕቃዎች፣ መግለጫዎች፣ ሜታዳታ፣ ኦዲዮ)።
- በመላ ኦፕሬተሮች ላይ የመወሰን አፈፃፀም ዋስትና-ቋሚ ኦሴት ፣ ዘር
  ማስመሰያ፣ የታሰሩ ትክክለኛነት እና የተስተካከሉ ቅርሶች።
- ለኦዲት ዝግጁ የሆኑ ቅርሶችን ማምረት፡- መግለጫዎች፣ የውጤት ካርዶች፣ የመለኪያ ማስረጃዎች፣
  እና ግልጽነት መፍጨት አስተዳደር DAG ውስጥ ለህትመት ተስማሚ.
- Surface telemetry ስለዚህ SREs ተንሳፋፊነትን፣ የውሸት አወንታዊ ጉዳዮችን እና የእረፍት ጊዜን መለየት ይችላሉ።
  ጥሬ የተጠቃሚ ውሂብ ሳይሰበስብ.

## 2. ቆራጥ የአፈፃፀም ውል
- ** Runtime:** ONNX Runtime 1.19.x (CPU backend) ከ AVX2 ተሰናክሏል እና
  የኦፕኮድ ስብስብን ለመጠገን `--enable-extended-minimal-build`። CUDA/ብረት
  Runtimes በምርት ላይ በግልፅ ተከልክሏል።
- ** ተቃርኖ: *** `opset=17`. አዳዲስ ኦፕሴቶችን የሚያነጣጥሩ ሞዴሎች ወደ ታች መቀየር አለባቸው
  እና ከመግባቱ በፊት የተረጋገጠ.
- **የዘር መመንጨት፡** እያንዳንዱ ግምገማ የ RNG ዘርን ያገኘው ከ ነው።
  `BLAKE3(content_digest || manifest_id || run_nonce)` የሚመጣው `run_nonce`
  ከአስተዳደር-ከተፈቀደው አንጸባራቂ. ዘሮች ሁሉንም የስቶክቲክ ክፍሎችን ይመገባሉ
  (የጨረር ፍለጋ፣ ማቋረጥ መቀያየር) ስለዚህ ውጤቶቹ ቢት-ለ-ቢት ሊባዙ ይችላሉ።
- ** ክር: በአንድ ሞዴል አንድ ሠራተኛ. Concurrency በሩጫው የተቀናጀ ነው።
  የጋራ-ግዛት የዘር ሁኔታዎችን ለማስወገድ ኦርኬስትራ። የBLAS ቤተ-መጻሕፍት ይሠራሉ
  ነጠላ-ክር ሁነታ.
- ** ቁጥሮች: ** FP16 ማከማቸት የተከለከለ ነው. FP32 መሃከለኛዎችን እና መቆንጠጫ ይጠቀሙ
  ከመደመር በፊት ወደ አራት የአስርዮሽ ቦታዎች ይወጣል።

## 3. የኮሚቴ ቅንብር
የመነሻ ኮሚቴው ሶስት ሞዴል ቤተሰቦችን ይዟል። አስተዳደር ሊጨምር ይችላል።
ሞዴሎች፣ ነገር ግን ዝቅተኛው ምልአተ ጉባኤ ረክቶ መቆየት አለበት።

| ቤተሰብ | ቤዝላይን ሞዴል | ዓላማ |
|--------|----------|
| ራዕይ | ክፍት CLIP ViT-H/14 (የደህንነት በጥሩ ሁኔታ የተስተካከለ) | ምስላዊ የኮንትሮባንድ፣ ብጥብጥ፣ የCSAM አመልካቾችን ያገኛል። |
| መልቲሞዳል | LLaVA-1.6 34B ደህንነት | የጽሑፍ + የምስል መስተጋብርን፣ የአውድ ምልክቶችን፣ ትንኮሳን ይይዛል። |
| አስተዋይ | pHash + aHash + NeuralHash-ላይት ስብስብ | የታወቁ መጥፎ ነገሮችን በፍጥነት ማግኘት እና ማስታወስ። |

እያንዳንዱ ሞዴል ግቤት የሚከተሉትን ይገልጻል:
- `model_id` (UUID)
- `artifact_digest` (BLAKE3-256 የ OCI ምስል)
- `weights_digest` (BLAKE3-256 የ ONNX ወይም የተዋሃዱ ሴፍቴንስተሮች ብሎብ)
- `opset` (`17` እኩል መሆን አለበት)
- `weight` (የኮሚቴ ክብደት፣ ነባሪ `1.0`)
- `critical_labels` (ወዲያውኑ `Escalate` የሚያነቃቁ የመለያዎች ስብስብ)
- `max_eval_ms` (ለወሰኑ ጠባቂዎች መከላከያ)

## 4. I18NT0000001X መግለጫዎች እና ውጤቶች

### 4.1 የኮሚቴ መግለጫ
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

### 4.2 የግምገማ ውጤት
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

ሯጩ የሚወስን `AiModerationDigestV1` (BLAKE3 በላይ
ተከታታይ ውጤት) ለግልጽነት ምዝግብ ማስታወሻዎች እና ውጤቶችን ወደ ልከኝነት ጨምረው
ፍርዱ `pass` አይደለም ጊዜ ledger.

### 4.3 አድቨርሳሪያል ኮርፐስ ማንፌስት

የጌትዌይ ኦፕሬተሮች አሁን ግንዛቤን የሚገልጽ ተጓዳኝ መግለጫ ያስገባሉ።
ከካሊብሬሽን ሩጫዎች የተገኘ ሃሽ/ማካተት “ቤተሰብ”

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

መርሃግብሩ በI18NI0000031X ውስጥ ይኖራል እና ነው።
በ `AdversarialCorpusManifestV1::validate()` በኩል የተረጋገጠ. አንጸባራቂው ይፈቅዳል
የጌትዌይ መካድ ጫኚ የ `perceptual_family` ግቤቶችን የሚያግድ
ከግለሰብ ባይት ይልቅ ሙሉ በሙሉ የተባዙ ስብስቦች። ሊሮጥ የሚችል መሣሪያ
(`docs/examples/ai_moderation_perceptual_registry_202602.json`) ያሳያል
የሚጠበቀው አቀማመጥ እና በቀጥታ ወደ ናሙና ጌትዌይ መካድ ዝርዝር ውስጥ ይመገባል።

## 5. የማስፈጸሚያ ቧንቧ
1. ከአስተዳደሩ DAG I18NI0000035X ይጫኑ. ከሆነ ውድቅ ያድርጉ
   `runner_hash` ወይም `runtime_version` ከተዘረጋው ሁለትዮሽ ጋር አይዛመድም።
2. የሞዴል ቅርሶችን በኦሲአይ ዲጀስት በኩል ያውጡ፣ ከመጫንዎ በፊት የምግብ መፈጨትን ያረጋግጡ።
3. የግምገማ ስብስቦችን በይዘት አይነት መገንባት; ማዘዝ መደርደር አለበት።
   `(content_digest, manifest_id)` መወሰኛ ድምርን ለማረጋገጥ።
4. እያንዳንዱን ሞዴል በተገኘው ዘር ያስፈጽም. ለአስተዋይ ሃሽ፣ አጣምር
   ስብስብ በድምጽ ብልጫ -> ውጤት በ`[0,1]`።
5. የተመጣጠነ የተቀነጨበ ሬሾን በመጠቀም ውጤቶችን ወደ `combined_score` ያዋህዱ።
   ```
   combined = Σ_i weight_i * clamp(score_i / threshold_i, 0, 1) / Σ_i weight_i
   ```
6. `ModerationVerdictV1` አምር፡
   - `escalate` ማንኛውም `critical_labels` እሳት ወይም I18NI0000044X ከሆነ.
   - `quarantine` ከ `thresholds.quarantine` በላይ ከሆነ ግን ከ `escalate` በታች ከሆነ።
   - `pass` አለበለዚያ.
7. `AiModerationResultV1`ን ቀጥል እና የታችኛውን ተፋሰስ ሂደቶችን አሰልፍ፡
   - የኳራንቲን አገልግሎት (ፍርዱ ከተባባሰ/የገለልተኛ አካል)
   - ግልጽነት የምዝግብ ማስታወሻ ጸሐፊ (`ModerationLedgerV1`)
   - ቴሌሜትሪ ላኪ

## 6. የካሊብሬሽን እና ግምገማ
- ** የውሂብ ስብስቦች፡** የመሠረት ልኬት በፖሊሲ የተዘጋጀውን ድብልቅ ኮርፐስ ይጠቀማል
  የቡድን ይሁንታ. ማጣቀሻ በ `calibration_dataset` ውስጥ ተመዝግቧል።
- ** መለኪያዎች፡** የብሪየር ነጥብ፣ የሚጠበቀው የካሊብሬሽን ስህተት (ECE) እና AUROC ያሰሉ
  በአንድ ሞዴል እና ጥምር ፍርድ. ወርሃዊ ተሃድሶ መጠበቅ አለበት
  `Brier ≤ 0.18` እና `ECE ≤ 0.05`። ውጤቶች በ I18NT0000004X ሪፖርቶች ዛፍ ውስጥ ተከማችተዋል።
  (ለምሳሌ፣ [የካቲት 2026 ልኬት](../sorafs/reports/ai-moderation-calibration-202602.md))።
- ** መርሐግብር:** ወርሃዊ ተሃድሶ (የመጀመሪያው ሰኞ)። የአደጋ ጊዜ ማስተካከያ
  ተንሸራታች ማንቂያዎች ከተቃጠሉ ይፈቀዳል.
- ** ሂደት: ** በመለኪያ ስብስብ ላይ የመወሰን ግምገማ ቧንቧን ያካሂዱ ፣
  `thresholds` እንደገና ማመንጨት፣ አንጸባራቂን ማዘመን፣ የአስተዳደር ድምጽ ደረጃ ለውጦች።

## 7. ማሸግ እና ማሰማራት
- በ I18NI0000055X በኩል የ OCI ምስሎችን ይገንቡ።
- ምስሎች የሚከተሉትን ያካትታሉ:
  - የተቆለፈ Python env (`poetry.lock`) ወይም Rust binary `Cargo.lock`።
  - `models/` ማውጫ ከኦኤንኤንኤክስ ክብደት ጋር።
  - የመግቢያ ነጥብ I18NI0000059X (ወይም ዝገት አቻ) HTTP/gRPC ኤፒአይን የሚያጋልጥ።
- ቅርሶችን ወደ `registry.sora.net/ministry/ai-moderation/<model>@sha256:<digest>` ያትሙ።
- ሯጭ ሁለትዮሽ መርከቦች እንደ `sorafs_ai_runner` crate አካል። የግንባታ ቧንቧ መስመር
  አንጸባራቂ hash በሁለትዮሽ ውስጥ (በ`/v2/info` የተጋለጠ) ያስገባል።

## 8. ቴሌሜትሪ እና ታዛቢነት
- Prometheus መለኪያዎች፡
  - `moderation_requests_total{verdict}`
  - `moderation_model_score_bucket{model_id,label}`
  - `moderation_combined_score_bucket`
  - `moderation_inference_latency_seconds_bucket`
  - `moderation_runner_manifest_info{manifest_id, runtime_version}`
- ምዝግብ ማስታወሻዎች፡- JSON መስመሮች ከ `request_id`፣ `manifest_id`፣ `verdict`፣ እና መፍጨት
  የተከማቸ ውጤት. ጥሬ ውጤቶች በምዝግብ ማስታወሻዎች ውስጥ ወደ ሁለት አስርዮሽ ቦታዎች ተቀይረዋል።
- በ I18NI0000071X ውስጥ የተከማቹ ዳሽቦርዶች
  (ከመጀመሪያው የካሊብሬሽን ሪፖርት ጎን ለጎን የታተመ)።
- የማንቂያ ገደቦች;
  - የጠፋ መግቢያ (I18NI0000072X ለ 10 ደቂቃዎች ቆሟል).
  - ተንሸራታች ማወቂያ (አማካይ የሞዴል ነጥብ ዴልታ>20% ከጥቅል ባለ 7-ቀን አማካኝ ጋር)።
  - የውሸት-አዎንታዊ መዝገብ (የኳራንቲን ወረፋ> 50 እቃዎች ለ> 30 ደቂቃዎች).

## 9. አስተዳደር እና ለውጥ ቁጥጥር
መግለጫዎች ድርብ ፊርማ ያስፈልጋቸዋል፡ የሚኒስቴር ምክር ቤት አባል + አወያይ SRE
  መምራት በ I18NI0000073X ውስጥ የተመዘገቡ ፊርማዎች።
- ለውጦች ከ I18NI0000074X እስከ I18NT0000008X ይከተላሉ። Hashes
  ወደ አስተዳደር DAG ገባ; ፕሮፖዛሉ እስኪሆን ድረስ ማሰማራት ታግዷል
  ተደነገገ።
- ሯጭ ሁለትዮሽ `runner_hash` መክተት; hashes ቢለያይ CI ማሰማራትን አይቀበልም።
- ግልጽነት: በየሳምንቱ I18NI0000076X ማጠቃለያ መጠን, የፍርድ ድብልቅ,
  እና ይግባኝ ውጤቶች. ለሶራ ፓርላማ ፖርታል ታትሟል።

## 10. ደህንነት እና ግላዊነት
- የይዘት መፍቻዎች BLAKE3 ይጠቀማሉ። ጥሬ ሸክሞች ከኳራንቲን ውጭ አይቆዩም።
- የኳራንቲን መዳረሻ በጊዜ-ጊዜ ማጽደቅን ይፈልጋል። ሁሉም መዳረሻዎች ገብተዋል።
- ሯጭ ማጠሪያ ያልታመነ ይዘትን፣ 512 ሚቢ የማህደረ ትውስታ ገደቦችን እና 120 ሴ.
  የግድግዳ ሰዓት ጠባቂዎች.
- ልዩነት ግላዊነት እዚህ አይተገበርም; መግቢያዎች በኳራንቲን + ኦዲት ላይ ጥገኛ ናቸው።
  በምትኩ የስራ ፍሰቶች. የማሻሻያ ፖሊሲዎች የመተላለፊያ መንገድ ተገዢነትን እቅድ ይከተላሉ
  (`docs/source/sorafs_gateway_compliance_plan.md`፤ ፖርታል ቅጂ በመጠባበቅ ላይ)።

## 11. የካሊብሬሽን ህትመት (2026-02)
- ** ገላጭ: ** `docs/examples/ai_moderation_calibration_manifest_202602.json`
  በአስተዳደር የተፈረመውን `AiModerationManifestV1` (መታወቂያ
  `c9bdf0b2-63a3-4a90-8d70-908d119c2c7e`) ፣ የውሂብ ስብስብ ማጣቀሻ
  `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`፣ ሯጭ ሃሽ
  `ea3c0fd0ff4bd4510e94c7c293b261f601cc0c4f9fbacd99b0401d233a7cdc20`፣ እና የ
  2026-02 የመለኪያ ገደቦች (`quarantine = 0.42`፣ `escalate = 0.78`)።
- ** የውጤት ሰሌዳ: *** `docs/examples/ai_moderation_calibration_scorecard_202602.json`
  በተጨማሪም በሰው ሊነበብ የሚችል ዘገባ በ
  `[SoraFS Reports › AI Moderation Calibration 2026-02](../sorafs/reports/ai-moderation-calibration-202602.md)`
  ለእያንዳንዱ ሞዴል Brier፣ ECE፣ AUROC እና የፍርድ ድብልቅን ይያዙ። የተዋሃዱ መለኪያዎች
  ዒላማዎቹን አሟልቷል (`Brier = 0.126`, `ECE = 0.034`).
- ** ዳሽቦርዶች እና ማንቂያዎች፡** `dashboards/grafana/ministry_moderation_overview.json`
  እና `dashboards/alerts/ministry_moderation_rules.yml` (ከድጋሚ ሙከራዎች ጋር በ
  `dashboards/alerts/tests/ministry_moderation_rules.test.yml`) ያቅርቡ
  ልከኝነት ወደ ውስጥ መግባት/የማዘግየት/ተንሸራታች ክትትል ታሪክ ለመልቀቅ ያስፈልጋል።## 12. የመድገም እቅድ እና አረጋጋጭ (MINFO-1b)
- ቀኖናዊ I18NT0000002X ዓይነቶች አሁን ከተቀረው የ SoraFS እቅድ ጋር አብረው ይኖራሉ
  `crates/iroha_data_model/src/sorafs/moderation.rs`. የ
  `ModerationReproManifestV1`/I18NI0000094X መዋቅሮች
  አንጸባራቂ UUID፣ ሯጭ ሃሽ፣ የሞዴል መፋፈሻዎች፣ የመግቢያ ገደብ እና የዘር ቁሳቁስ።
  `ModerationReproManifestV1::validate` የመርሃግብር ስሪት ያስፈጽማል
  (`MODERATION_REPRO_MANIFEST_VERSION_V1`)፣ እያንዳንዱ አንጸባራቂ መያዙን ያረጋግጣል
  ቢያንስ አንድ ሞዴል እና ፈራሚ፣ እና እያንዳንዱን `SignatureOf<ModerationReproBodyV1>` ያረጋግጣል
  በማሽን ሊነበብ የሚችል ማጠቃለያ ከመመለስዎ በፊት።
- ኦፕሬተሮች የተጋራውን አረጋጋጭ በ በኩል መጥራት ይችላሉ።
  `sorafs_cli moderation validate-repro --manifest=PATH [--format=json|norito]`
  (በI18NI0000099X ውስጥ የተተገበረ)። CLI
  ከታች የታተሙትን የJSON ቅርሶችን ይቀበላል
  `docs/examples/ai_moderation_calibration_manifest_202602.json` ወይም ጥሬው።
  Norito ኢንኮዲንግ እና የሞዴል/ፊርማ ቆጠራዎችን ከማንፀባረቂያው ጎን ያትማል።
  የጊዜ ማህተም ማረጋገጫ አንዴ ከተሳካ።
- የመተላለፊያ መንገዶች እና አውቶማቲክ መንጠቆዎች ወደ አንድ አይነት ረዳት ስለሚገቡ መራባት ይገለጣል
  መርሃግብሮች ሲንሸራተቱ፣ የምግብ መፍጫ አካላት ሲጎድሉ ወይም በቆራጥነት ውድቅ ሊደረግ ይችላል።
  ፊርማዎች ማረጋገጥ አልተሳካም.
- Adversarial corpus ጥቅሎች ተመሳሳይ ንድፍ ይከተላሉ፡-
  `sorafs_cli moderation validate-corpus --manifest=PATH [--format=json|norito]`
  `AdversarialCorpusManifestV1`ን ይተነትናል፣ የመርሃግብር ስሪቱን ያስፈጽማል እና እምቢ አለ።
  ቤተሰቦችን፣ ተለዋጮችን ወይም የጣት አሻራ ዲበ ውሂብን የሚተውን ያሳያል። ስኬታማ
  የወጣውን በጊዜ ማህተም፣ የቡድን መለያ እና ቤተሰብ/ተለዋጭ ቆጠራዎችን ያወጣል።
  ስለዚህ ኦፕሬተሮች የመተላለፊያ መንገዱን ውድቅ መዝገብ ከማዘመንዎ በፊት ማስረጃውን ማያያዝ ይችላሉ።
  በክፍል 4.3 ውስጥ ተገልጿል.

## 13. ክትትሎችን ክፈት
- ከ2026-03-02 በኋላ ወርሃዊ የማገገሚያ መስኮቶች መከተላቸውን ቀጥለዋል።
  በክፍል 6 ውስጥ ያለው አሰራር; `ai-moderation-calibration-<YYYYMM>.md` አትም
  በ SoraFS ሪፖርቶች ዛፍ ስር ከተዘመኑ የማኒፌክት/የነጥብ ካርድ ቅርቅቦች ጋር።
- MINFO-1b እና MINFO-1c (የመባዛት አንጸባራቂ አረጋጋጮች እና ተቃዋሚዎች
  ኮርፐስ መዝገብ ቤት) በፍኖተ ካርታው ላይ በተናጠል ክትትል ይደረግበታል።