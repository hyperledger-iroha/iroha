---
lang: my
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

# AI Moderation Runner Specification

ဤသတ်မှတ်ချက်သည် **MINFO-1 — AI တည်ထောင်ခြင်း၏ စာရွက်စာတမ်းအပိုင်းကို ဖြည့်ဆည်းပေးပါသည်။
ထိန်းညှိမှုအခြေခံ**။ ၎င်းသည် အဆုံးအဖြတ်ပေးသည့် အကောင်အထည်ဖော်မှု စာချုပ်ကို သတ်မှတ်သည်။
ဂိတ်ဝတိုင်း တူညီစွာ လည်ပတ်နိုင်စေရန် ပြန်ကြားရေး ဝန်ကြီးဌာနမှ စိစစ်ဆောင်ရွက်ပေးပါသည်။
အယူခံဝင်မှုနှင့် ပွင့်လင်းမြင်သာမှု စီးဆင်းမှု (SFM-4/SFM-4b) မတိုင်မီ ပိုက်လိုင်းများ။ အပြုအမူအားလုံး
ဤနေရာတွင် ဖော်ပြထားသည်မှာ သတင်းအချက်အလတ်အဖြစ် အတိအလင်း မှတ်သားထားခြင်းမရှိပါက စံနှုန်းဖြစ်သည်။

## 1. Goals & Scope
- ဂိတ်ဝေးအကြောင်းအရာကို အကဲဖြတ်သည့် ပြန်လည်ထုတ်လုပ်နိုင်သော ပြန်လည်စိစစ်ရေးကော်မတီကို ပေးပါ။
  ကွဲပြားသော ပုံစံများကို အသုံးပြု၍ (အရာဝတ္ထုများ၊ ဖော်ပြချက်များ၊ မက်တာဒေတာ၊ အသံ)။
- အော်ပရေတာများတစ်လျှောက် အဆုံးအဖြတ်ပေးသော ကွပ်မျက်မှုကို အာမခံသည်- ပုံသေ opset၊ မျိုးစေ့ချထားသည်။
  တိုကင်ယူမှု၊ တိကျမှု၊ နှင့် ဗားရှင်းဖန်တီးထားသော အရာများ။
- စာရင်းစစ်- အဆင်သင့်ဖြစ်သောအရာများကို ထုတ်လုပ်ပါ- သရုပ်ပြမှုများ၊ အမှတ်စာရင်းများ၊ ချိန်ညှိမှု အထောက်အထားများ၊
  အုပ်ချုပ်ရေး DAG တွင် ထုတ်ဝေရန် သင့်လျော်သော ပွင့်လင်းမြင်သာမှု အချေအတင်များ။
- Surface telemetry သည် SRE များသည် ပျံ့လွင့်မှု၊ မှားယွင်းသော အပြုသဘောဆောင်မှုများနှင့် စက်ရပ်နေချိန်တို့ကို သိရှိနိုင်မည်ဖြစ်သည်။
  အသုံးပြုသူဒေတာအကြမ်းမစုဆောင်းဘဲ။

## 2. Deterministic Execution စာချုပ်
- **Runtime:** ONNX Runtime 1.19.x (CPU backend) ကို AVX2 ပိတ်ထားပြီး၊
  opcode set ကို fix ထားရန် `--enable-extended-minimal-build`။ CUDA/သတ္တု
  runtime များကို ထုတ်လုပ်မှုတွင် အထူးတလည် ခွင့်မပြုပါ။
- **Opset-** `opset=17`။ ပိုစ့်အသစ်များကို ပစ်မှတ်ထားသော မော်ဒယ်များကို လျှော့-ပြောင်းရပါမည်။
  နှင့် ဝင်ခွင့်မပြုမီ အတည်ပြုခဲ့သည်။
- **မျိုးစေ့မှဆင်းသက်လာခြင်း-** အကဲဖြတ်မှုတိုင်းသည် RNG မျိုးစေ့မှ ဆင်းသက်လာသည်။
  `BLAKE3(content_digest || manifest_id || run_nonce)` ရှိရာ `run_nonce` လာသည်
  အုပ်ချုပ်မှုမှ အတည်ပြုထားသော သရုပ်ပြမှု။ အစေ့များသည် stochastic အစိတ်အပိုင်းအားလုံးကို အစာကျွေးသည်။
  (အလင်းတန်းရှာဖွေမှု၊ ကျောင်းထွက်ခလုတ်များ) ထို့ကြောင့် ရလဒ်များသည် တစ်နည်းနည်းဖြင့် ပြန်ထုတ်နိုင်သည်။
- **Threading:** မော်ဒယ်တစ်ခုလျှင် အလုပ်သမားတစ်ဦး။ အပြေးသမားက ညှိနှိုင်းပေးသည်။
  shared-state ပြိုင်ပွဲအခြေအနေများကိုရှောင်ရှားရန် orchestrator ။ BLAS စာကြည့်တိုက်များ လည်ပတ်နေသည်။
  single-threaded မုဒ်။
- ** ကိန်းဂဏာန်းများ-** FP16 စုဆောင်းခြင်းကို တားမြစ်ထားသည်။ FP32 ကြားခံကိရိယာများနှင့် ကလစ်ကိုသုံးပါ။
  ပေါင်းစည်းခြင်းမပြုမီ ဒဿမ လေးနေရာသို့ အထွက်။

## 3. ကော်မတီဖွဲ့စည်းမှု
အခြေခံကော်မတီတွင် စံပြမိသားစုသုံးစု ပါဝင်သည်။ အုပ်ချုပ်မှု ထည့်နိုင်သည်။
မော်ဒယ်များ ဖြစ်သော်လည်း အနိမ့်ဆုံး အထမြောက်မှုကို ကျေနပ်နေရပါမည်။

| မိသားစု | အခြေခံမော်ဒယ် | ရည်ရွယ်ချက် |
|--------|----------------|---------|
| အမြင် | OpenCLIP ViT-H/14 (ဘေးကင်းအောင် ချိန်ညှိထားသည်) | အမြင်အာရုံ ဆန့်ကျင်မှု၊ အကြမ်းဖက်မှု၊ CSAM အညွှန်းများကို ထောက်လှမ်းသည်။ |
| ဘက်စုံ | LLaVA-1.6 34B လုံခြုံမှု | စာသား + ရုပ်ပုံ အပြန်အလှန်တုံ့ပြန်မှုများ၊ ဆက်စပ်အချက်များ၊ နှောင့်ယှက်မှုများကို ဖမ်းယူသည်။ |
| သိမြင်နိုင်စွမ်း | pHash + aHash + NeuralHash-lite အဖွဲ့ | မိတ္တူပွားလုနီးကို လျင်မြန်စွာ ရှာဖွေတွေ့ရှိပြီး လူသိများသော မကောင်းတဲ့ပစ္စည်းကို ပြန်လည်သိမ်းဆည်းပါ။ |

မော်ဒယ်ထည့်သွင်းမှုတစ်ခုစီကို သတ်မှတ်သည်-
- `model_id` (UUID)
- `artifact_digest` (OCI ပုံ၏ BLAKE3-256)
- `weights_digest` (ONNX ၏ BLAKE3-256 သို့မဟုတ် ပေါင်းစည်းထားသော ဘေးကင်းစေသော blob)
- `opset` (`17` နှင့် ညီမျှရမည်)
- `weight` (ကော်မတီအလေးချိန်၊ မူရင်း `1.0`)
- `critical_labels` (`Escalate` ကို ချက်ချင်း အစပျိုးပေးမယ့် တံဆိပ်များ အစုံ)
- `max_eval_ms` (သတ်မှတ်ထားသော စောင့်ကြည့်ခွေးများအတွက် အကာအရံများ)

## 4. Norito Manifests & Results

### ၄.၁ ကော်မတီ ကြေငြာချက်
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

### 4.2 အကဲဖြတ်ခြင်းရလဒ်
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

အပြေးသမားသည် အဆုံးအဖြတ်ပေးသော `AiModerationDigestV1` (BLAKE3 ကို အပေါ်မှ ထုတ်လွှတ်ရမည်)
serialized ရလဒ်) ပွင့်လင်းမြင်သာမှုမှတ်တမ်းများနှင့် ရလဒ်များကို ထိန်းညှိမှုတွင် ပေါင်းထည့်ရန်
စီရင်ချက်သည် `pass` မဟုတ်သည့်အခါ စာရင်းဇယား။

### 4.3 Adversarial Corpus Manifest

ယခုအခါတွင် Gateway အော်ပရေတာများသည် အာရုံခံစားမှုကို ရေတွက်သည့် အဖော် manifest တစ်ခုကို စားသုံးနေပါသည်။
ချိန်ညှိခြင်းလုပ်ဆောင်ခြင်းမှ ဆင်းသက်လာသော hash/embedding "မိသားစုများ"

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

schema သည် `crates/iroha_data_model/src/sorafs/moderation.rs` တွင်နေထိုင်သည်။
`AdversarialCorpusManifestV1::validate()` မှတစ်ဆင့် တရားဝင်စစ်ဆေးခဲ့သည်။ Manifest က ခွင့်ပြုတယ်။
ပိတ်ဆို့သော `perceptual_family` ကိုဖြည့်သွင်းရန် gateway denylist loader
တစ်ခုချင်း bytes အစား အနီးရှိ-ပွားနေသော အစုအဝေးကြီးတစ်ခုလုံး။ ပြေးနိုင်သော ခံစစ်မှူး
(`docs/examples/ai_moderation_perceptual_registry_202602.json`) သရုပ်ပြ
မျှော်လင့်ထားသည့် အပြင်အဆင်ကို နမူနာ gateway ငြင်းပယ်စာရင်းသို့ တိုက်ရိုက် ပေးပို့ပါသည်။

## 5. Execution Pipeline
1. အုပ်ချုပ်မှု DAG မှ `AiModerationManifestV1` ကို တင်ပါ။ ငြင်းဆိုပါ။
   `runner_hash` သို့မဟုတ် `runtime_version` သည် ဖြန့်ကျက်ထားသည့် ဒွိစုံနှင့် မကိုက်ညီပါ။
2. မတင်ပေးမီ အချေအတင်များကို စစ်ဆေးခြင်း OCI digest မှတစ်ဆင့် မော်ဒယ်လ်ပစ္စည်းများကို ရယူပါ။
3. အကြောင်းအရာအမျိုးအစားအလိုက် အကဲဖြတ်ခြင်းအတွဲများကို တည်ဆောက်ပါ။ အော်ဒါမှာ အမျိုးအစားခွဲရပါမယ်။
   အဆုံးအဖြတ်ရှိသော စုစည်းမှုကို သေချာစေရန် `(content_digest, manifest_id)`။
4. ရရှိလာသောမျိုးစေ့ဖြင့် မော်ဒယ်တစ်ခုစီကို အကောင်အထည်ဖော်ပါ။ သိမြင်နိုင်သော ကိန်းဂဏန်းများ အတွက် ပေါင်းစပ်ပါ။
   လူများစုမဲမှ တစ်ဆင့် -> `[0,1]` တွင် ရမှတ်များ။
5. အလေးချိန်ညှပ်ထားသောအချိုးကို အသုံးပြု၍ `combined_score` တွင် စုစည်းထားသောရမှတ်များ-
   ```
   combined = Σ_i weight_i * clamp(score_i / threshold_i, 0, 1) / Σ_i weight_i
   ```
6. ထုတ်လုပ်သည့် `ModerationVerdictV1`-
   - `escalate` ရှိပါက `critical_labels` သို့မဟုတ် `combined ≥ thresholds.escalate`။
   - `quarantine` `thresholds.quarantine` အထက်ဆိုလျှင် `escalate` အောက်။
   - `pass` မဟုတ်ရင်။
7. `AiModerationResultV1` ကို ဆက်တည်ထားပြီး ရေအောက်ပိုင်း လုပ်ငန်းစဉ်များကို စီစစ်ပါ-
   - Quarantine Service (စီရင်ချက်တိုးလာပါက/quarantines)
   - ပွင့်လင်းမြင်သာမှုမှတ်တမ်းရေးသူ (`ModerationLedgerV1`)
   - Telemetry တင်ပို့သူ

## 6. Calibration & Evaluation
- **ဒေတာအတွဲများ-** အခြေခံစံကိုက်ချိန်ညှိမှုသည် မူဝါဒဖြင့် ပေါင်းစပ်ထားသော ပေါင်းစပ်ဖွဲ့စည်းပုံကို အသုံးပြုသည်။
  အဖွဲ့၏ခွင့်ပြုချက်။ `calibration_dataset` တွင် မှတ်တမ်းတင်ထားသော အကိုးအကား။
- **Metrics-** Compute Brier ရမှတ်၊ မျှော်လင့်ထားသော Calibration Error (ECE) နှင့် AUROC
  မော်ဒယ်တစ်ခုစီနှင့် ပေါင်းစပ်စီရင်ချက်။ လစဉ်ပြန်လည်ချိန်ညှိမှုကို ထားရှိရမည်။
  `Brier ≤ 0.18` နှင့် `ECE ≤ 0.05`။ SoraFS အစီရင်ခံစာသစ်ပင်တွင် သိမ်းဆည်းထားသော ရလဒ်များ
  (ဥပမာ၊ [ဖေဖော်ဝါရီလ 2026 စံကိုက်ညှိခြင်း](../sorafs/reports/ai-moderation-calibration-202602.md))။
- **အချိန်ဇယား-** လစဉ်ပြန်လည်ချိန်ညှိခြင်း (ပထမတနင်္လာနေ့)။ အရေးပေါ်ပြန်လည်ချိန်ညှိခြင်း။
  ရေပျံသတိပေးချက်များ မီးလောင်လျှင် ခွင့်ပြုသည်။
- ** လုပ်ငန်းစဉ်-** စံသတ်မှတ်ထားသော အကဲဖြတ်မှု ပိုက်လိုင်းကို စံကိုက်သတ်မှတ်မှုတွင် လုပ်ဆောင်ပါ၊
  `thresholds` ကို ပြန်ထုတ်ပါ၊ မန်နီးဖက်စ်ကို အပ်ဒိတ်လုပ်ပါ၊ အုပ်ချုပ်ရေးမဲအတွက် အဆင့်ပြောင်းလဲမှုများ။

## 7. ထုပ်ပိုးခြင်းနှင့် ဖြန့်ကျက်ခြင်း။
- `docker buildx bake -f docker/ai_moderation.hcl` မှတဆင့် OCI ရုပ်ပုံများကိုတည်ဆောက်ပါ။
- ပုံများပါဝင်သည်-
  - လော့ခ်ချထားသော Python env (`poetry.lock`) သို့မဟုတ် Rust binary `Cargo.lock`။
  - ONNX အလေးများပါသော `models/` လမ်းညွှန်။
  - HTTP/gRPC API ကို ဖော်ထုတ်သည့် ဝင်ခွင့်အမှတ် `run_moderation.py` (သို့မဟုတ် သံချေးတက်ခြင်းနှင့် ညီမျှသည်)။
- ရှေးဟောင်းပစ္စည်းများကို `registry.sora.net/ministry/ai-moderation/<model>@sha256:<digest>` သို့ ထုတ်ဝေပါ။
- `sorafs_ai_runner` သေတ္တာ၏တစ်စိတ်တစ်ပိုင်းအဖြစ် Runner binary သင်္ဘောများ။ ပိုက်လိုင်းတည်ဆောက်သည်။
  binary တွင် manifest hash ကို မြှုပ်နှံသည် (`/v2/info` မှတဆင့် ဖော်ထုတ်သည်)။

## 8. Telemetry & Observability
- Prometheus မက်ထရစ်များ
  - `moderation_requests_total{verdict}`
  - `moderation_model_score_bucket{model_id,label}`
  - `moderation_combined_score_bucket`
  - `moderation_inference_latency_seconds_bucket`
  - `moderation_runner_manifest_info{manifest_id, runtime_version}`
- မှတ်တမ်းများ- `request_id`၊ `manifest_id`၊ `verdict` ပါသော JSON လိုင်းများနှင့် မှတ်တမ်းများ
  သိမ်းဆည်းထားသောရလဒ်။ အကြမ်းမှတ်များကို မှတ်တမ်းများတွင် ဒဿမနှစ်နေရာသို့ ပြန်ပြောင်းသည်။
- `dashboards/grafana/ministry_moderation_overview.json` တွင် သိမ်းဆည်းထားသော ဒိုင်ခွက်များ
  (ပထမအဆင့် တိုင်းတာမှုအစီရင်ခံစာနှင့်အတူ ထုတ်ဝေခဲ့သည်)။
- သတိပေးချက်အဆင့်များ-
  - စားသုံးမိခြင်း ပျောက်ဆုံးခြင်း (`moderation_requests_total` ကို 10 မိနစ်ကြာ ရပ်ထားသည်)။
  - Drift detection (ပျမ်းမျှမော်ဒယ်ရမှတ် မြစ်ဝကျွန်းပေါ်ဒေသ > 20% နှင့် 7-ရက်ပတ်လုံး လှိမ့်ခြင်းပျမ်းမျှ)။
  - မှားယွင်းသော-အပြုသဘောဆောင်သော မှတ်တမ်း (quarantine တန်းစီခြင်း > 50 မိနစ် > 30 ကြာ)။

## 9. Governance & Change Control
- သရုပ်ဖော်မှုတွင် လက်မှတ်နှစ်စောင် လိုအပ်သည်- ဝန်ကြီးဌာန ကောင်စီအဖွဲ့ဝင် + စိစစ်ရေး SRE
  ခဲ။ `AiModerationManifestV1.governance_signature` တွင် မှတ်တမ်းတင်ထားသော လက်မှတ်များ။
- အပြောင်းအလဲများသည် `ModerationManifestChangeProposalV1` မှ Torii အတိုင်း လိုက်လာပါသည်။ Hashes
  အုပ်ချုပ်မှု DAG သို့ ဝင်ရောက်ခဲ့သည်။ အဆိုပြုချက်မပြီးမချင်း ဖြန့်ကျက်ပိတ်ဆို့ထားသည်။
  ပြဋ္ဌာန်းခဲ့သည်။
- Runner binaries များသည် `runner_hash` ကို ထည့်သွင်းထားသည်။ hash ကွဲလွဲပါက CI သည် ဖြန့်ကျက်ခြင်းကို ငြင်းဆိုသည်။
- ပွင့်လင်းမြင်သာမှု- အပတ်စဉ် `ModerationScorecardV1` အကျဉ်းချုပ် အသံအတိုးအကျယ်၊ စီရင်ချက် ရောနှော၊
  အယူခံဝင်ခြင်း ရလဒ်များ။ Sora ပါလီမန်ပေါ်တယ်သို့ထုတ်ဝေခဲ့သည်။

## 10. လုံခြုံရေးနှင့် ကိုယ်ရေးကိုယ်တာ
- Content digests များသည် BLAKE3 ကိုအသုံးပြုသည်။ ကုန်ကြမ်း ဝန်ထုပ်ဝန်ပိုးများသည် quarantine အပြင်ဘက်တွင် ဘယ်သောအခါမှ မတည်မြဲပါ။
- quarantine သို့ဝင်ရောက်ခွင့်သည် အချိန်နှင့်တပြေးညီ အတည်ပြုချက်များ လိုအပ်သည်။ ဝင်ရောက်ခွင့်အားလုံးကို မှတ်တမ်းတင်ထားသည်။
- Runner sandbox များသည် စိတ်မချရသောအကြောင်းအရာများ၊ 512 MiB မှတ်ဉာဏ်ကန့်သတ်ချက်များနှင့် 120s ကို ပြဋ္ဌာန်းပေးသည်
  တိုင်ကပ်နာရီ အစောင့်များ
- Differential privacy ကို ဤနေရာတွင် မသုံးပါ။ ဂိတ်ဝေးများသည် quarantine + စာရင်းစစ်ကို အားကိုးသည်။
  အလုပ်အသွားအလာများအစား။ ပြန်လည်ပြုပြင်ရေးမူဝါဒများသည် တံခါးပေါက်လိုက်နာမှုအစီအစဉ်ကို လိုက်နာပါသည်။
  (`docs/source/sorafs_gateway_compliance_plan.md`; portal မိတ္တူကို ဆိုင်းငံ့ထားသည်)။

## 11. Calibration Publication (2026-02)
- **ဖော်ပြချက်-** `docs/examples/ai_moderation_calibration_manifest_202602.json`
  အုပ်ချုပ်မှု-လက်မှတ်ရေးထိုးထားသော `AiModerationManifestV1` (ID
  `c9bdf0b2-63a3-4a90-8d70-908d119c2c7e`), ဒေတာအတွဲကို ရည်ညွှန်းသည်။
  `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`၊ အပြေးသမား hash
  `ea3c0fd0ff4bd4510e94c7c293b261f601cc0c4f9fbacd99b0401d233a7cdc20` နှင့်
  2026-02 စံသတ်မှတ်ချက်များ (`quarantine = 0.42`၊ `escalate = 0.78`)။
- **ရမှတ်စာရင်း-** `docs/examples/ai_moderation_calibration_scorecard_202602.json`
  ထို့အပြင် လူသားဖတ်နိုင်သော အစီရင်ခံစာပါ
  `[SoraFS Reports › AI Moderation Calibration 2026-02](../sorafs/reports/ai-moderation-calibration-202602.md)`
  မော်ဒယ်တိုင်းအတွက် Brier၊ ECE၊ AUROC နှင့် စီရင်ချက် ရောနှောမှုကို ဖမ်းယူပါ။ ပေါင်းစပ်တိုင်းတာမှုများ
  ပစ်မှတ်များ (`Brier = 0.126`၊ `ECE = 0.034`) နှင့် ကိုက်ညီပါသည်။
- **ဒိုင်ခွက်များနှင့် သတိပေးချက်များ-** `dashboards/grafana/ministry_moderation_overview.json`
  နှင့် `dashboards/alerts/ministry_moderation_rules.yml` (ဆုတ်ယုတ်မှုစမ်းသပ်မှုများနှင့်အတူ
  `dashboards/alerts/tests/ministry_moderation_rules.test.yml`) ကိုပေးပါသည်။
  ထုတ်လွှင့်မှုအတွက် လိုအပ်သည်## 12. မျိုးပွားနိုင်မှု အစီအစဉ်နှင့် တရားဝင်စနစ် (MINFO-1b)
- Canonical Norito အမျိုးအစားများသည် ယခု SoraFS schema ၏ ကျန်ရှိသော အမျိုးအစားများနှင့်အတူ နေထိုင်သည်
  `crates/iroha_data_model/src/sorafs/moderation.rs`။ ဟိ
  `ModerationReproManifestV1`/`ModerationReproBodyV1` တည်ဆောက်ပုံများက ဖမ်းယူထားသည်
  UUID၊ runner hash၊ model digests၊ threshold set နှင့် seed material ကိုဖော်ပြပါ။
  `ModerationReproManifestV1::validate` သည် schema ဗားရှင်းကို ပြဌာန်းသည်။
  (`MODERATION_REPRO_MANIFEST_VERSION_V1`) သည် သရုပ်ပြမှုတိုင်းတွင် ရှိနေကြောင်း သေချာစေသည်။
  အနည်းဆုံး မော်ဒယ်နှင့် လက်မှတ်ရေးထိုးသူ `SignatureOf<ModerationReproBodyV1>` တစ်ခုစီကို အတည်ပြုသည်။
  စက်ဖတ်နိုင်သော အနှစ်ချုပ်ကို မပြန်မီ။
- အော်ပရေတာများသည် shared validator ကိုမှတဆင့်ခေါ်ဆိုနိုင်သည်။
  `sorafs_cli moderation validate-repro --manifest=PATH [--format=json|norito]`
  (`crates/sorafs_orchestrator/src/bin/sorafs_cli.rs` တွင် အကောင်အထည်ဖော်သည်)။ CLI
  အောက်တွင်ဖော်ပြထားသော JSON အနုပညာလက်ရာများကို လက်ခံသည်။
  `docs/examples/ai_moderation_calibration_manifest_202602.json` သို့မဟုတ် ကုန်ကြမ်း
  Norito ကို ကုဒ်သွင်းပြီး မန်နီးဖက်စ်နှင့်အတူ မော်ဒယ်/လက်မှတ် အရေအတွက်များကို ပရင့်ထုတ်သည်
  အတည်ပြုချက် အောင်မြင်သည်နှင့် တပြိုင်နက် အချိန်တံဆိပ်တုံး။
- Gateways နှင့် automation တို့သည် တူညီသောအထောက်အကူအဖြစ် ချိတ်ဆက်ထားသောကြောင့် မျိုးပွားနိုင်မှုကို ထင်ရှားစေသည်။
  အစီအစဥ်များ ပျံ့လွင့်သွားသည့်အခါ၊ အချေအတင်များ ပျောက်ဆုံးနေသည့်အခါ သို့မဟုတ် အဆုံးအဖြတ်ဖြင့် ငြင်းပယ်နိုင်သည်။
  လက်မှတ်များသည် စိစစ်မှု မအောင်မြင်ပါ။
- Adversarial corpus အစုအဝေးများသည် တူညီသောပုံစံအတိုင်း လိုက်နာကြသည်-
  `sorafs_cli moderation validate-corpus --manifest=PATH [--format=json|norito]`
  `AdversarialCorpusManifestV1` ကို ခွဲခြမ်းစိတ်ဖြာပြီး schema ဗားရှင်းကို ပြဌာန်းပြီး ငြင်းဆိုသည်
  မိသားစုများ၊ မျိုးကွဲများ သို့မဟုတ် လက်ဗွေ မက်တာဒေတာကို ချန်လှပ်ထားခြင်းကို ထင်ရှားစေသည်။ အောင်မြင်တယ်။
  လည်ပတ်မှုများသည် ထုတ်ပေးသည့်အချိန်တံဆိပ်၊ အစုအဝေးတံဆိပ်နှင့် မိသားစု/မူကွဲအရေအတွက်များကို ထုတ်လွှတ်သည်
  ထို့ကြောင့် အော်ပရေတာများသည် gateway denylist entries များကို မွမ်းမံပြင်ဆင်ခြင်းမပြုမီ အထောက်အထားကို ပင်ထိုးနိုင်ပါသည်။
  အပိုင်း 4.3 တွင်ဖော်ပြထားသည်။

## 13. နောက်ဆက်တွဲများကို ဖွင့်ပါ။
- 2026-03-02 ပြီးနောက် လစဉ်ပြန်လည်ချိန်ညှိမှုပြတင်းပေါက်များကို ဆက်လက်လိုက်နာပါ။
  ပုဒ်မ ၆ ပါ လုပ်ထုံးလုပ်နည်း၊ `ai-moderation-calibration-<YYYYMM>.md` ထုတ်ဝေသည်။
  SoraFS အစီရင်ခံစာသစ်ပင်အောက်ရှိ မွမ်းမံထားသော မန်နီးဖက်စ်/အမှတ်ပေးကတ်အတွဲများနှင့်အတူ။
- MINFO-1b နှင့် MINFO-1c (ပြန်ထုတ်နိုင်မှု ထင်ရှားသော သက်သေအထောက်အထားများ နှင့် ဆန့်ကျင်ဘက်
  corpus registry) လမ်းပြမြေပုံတွင် သီးခြားစီ ခြေရာခံနေပါသည်။