---
lang: uz
direction: ltr
source: docs/portal/docs/ministry/ai-moderation-runner.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: AI Moderation Runner Specification
summary: Deterministic moderation committee design for the Ministry of Information (MINFO-1) deliverable.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# AI Moderatsiya Runner spetsifikatsiyasi

Ushbu spetsifikatsiya **MINFO-1 — AI ni oʻrnatish hujjat qismiga javob beradi
moderatsiyaning asosiy darajasi**. U deterministik ijro shartnomasini belgilaydi
Axborot vazirligi moderatsiya xizmati, shuning uchun har bir shlyuz bir xil ishlashi mumkin
apellyatsiyalar va shaffoflik oqimlari oldidan quvurlar (SFM-4/SFM-4b). Barcha xatti-harakatlar
Bu yerda tasvirlanganlar, agar ma'lumot sifatida aniq belgilanmagan bo'lsa, normativ hisoblanadi.

## 1. Maqsadlar va qamrov
- Gateway tarkibini baholaydigan takrorlanadigan moderatsiya qo'mitasini taqdim eting
  (ob'ektlar, manifestlar, metadata, audio) heterojen modellar yordamida.
- Operatorlar bo'ylab deterministik bajarilishini kafolatlang: sobit opset, urug'langan
  tokenizatsiya, cheklangan aniqlik va versiyali artefaktlar.
- Auditga tayyor artefaktlarni yaratish: manifestlar, ko'rsatkichlar kartalari, kalibrlash dalillari,
  va boshqaruv DAGda nashr qilish uchun mos shaffoflik dayjestlari.
- Yuzaki telemetriya, shuning uchun SRElar drift, noto'g'ri pozitivlar va ishlamay qolish vaqtini aniqlay oladi
  foydalanuvchi ma'lumotlarini yig'masdan.

## 2. Deterministik ijro shartnomasi
- **Runtime:** ONNX Runtime 1.19.x (CPU backend) AVX2 o‘chirilgan va kompilyatsiya qilingan
  `--enable-extended-minimal-build` opcode to'plamini o'zgarmas holatda saqlash uchun. CUDA/Metal
  ishlab chiqarishda ishlash vaqtlari aniq taqiqlangan.
- **Opset:** `opset=17`. Yangi opsetlarga mo'ljallangan modellar pastga aylantirilishi kerak
  va qabul qilishdan oldin tasdiqlangan.
- **Urug'ning kelib chiqishi:** Har bir baholash RNG urug'ini oladi
  `BLAKE3(content_digest || manifest_id || run_nonce)` qaerda `run_nonce` keladi
  boshqaruv tomonidan tasdiqlangan manifestdan. Urug'lar barcha stokastik komponentlarni oziqlantiradi
  (nurni qidirish, o'chirish tugmalari) shuning uchun natijalar bitma-bit takrorlanishi mumkin.
- **Threading:** Har bir modelga bitta ishchi. Muvofiqlik yuguruvchi tomonidan muvofiqlashtiriladi
  umumiy shtat poyga shartlaridan qochish uchun orkestr. BLAS kutubxonalari ishlaydi
  bitta ipli rejim.
- **Raqamlar:** FP16 to'planishi taqiqlangan. FP32 oraliq mahsulotlardan foydalaning va qisqichni mahkamlang
  yig'ishdan oldin to'rtta kasrgacha chiqadi.

## 3. Qo'mita tarkibi
Asosiy qo'mita uchta namunaviy oilani o'z ichiga oladi. Boshqaruv qo'shishi mumkin
modellar, lekin minimal kvorum qoniqarli qolishi kerak.

| Oila | Asosiy model | Maqsad |
|--------|----------------|---------|
| Vizyon | OpenCLIP ViT-H/14 (xavfsizlik nozik sozlangan) | Vizual kontrabanda, zo'ravonlik, CSAM ko'rsatkichlarini aniqlaydi. |
| Multimodal | LLaVA-1.6 34B Xavfsizlik | Matn + rasm o'zaro ta'sirini, kontekstli signallarni, ta'qiblarni suratga oladi. |
| Pertseptiv | pHash + aHash + NeuralHash-lite ansambli | Ma'lum bo'lgan yomon materialni tez deyarli takroriy aniqlash va eslab qolish. |

Har bir model yozuvi quyidagilarni belgilaydi:
- `model_id` (UUID)
- `artifact_digest` (OCI tasvirining BLAKE3-256)
- `weights_digest` (ONNX-ning BLAKE3-256 yoki birlashtirilgan seyftensor bloklari)
- `opset` (`17` ga teng bo'lishi kerak)
- `weight` (qo'mitaning og'irligi, standart `1.0`)
- `critical_labels` (`Escalate` ni darhol ishga tushiradigan teglar to'plami)
- `max_eval_ms` (deterministik qo'riqchilar uchun panjara)

## 4. Norito Manifestlar va Natijalar

### 4.1 Qo'mitaning manifesti
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

### 4.2 Baholash natijasi
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

Yuguruvchi deterministik `AiModerationDigestV1` (BLAKE3) chiqarishi KERAK
ketma-ketlashtirilgan natija) shaffoflik jurnallari uchun va natijalarni moderatsiyaga qo'shing
hukm `pass` bo'lmasa, kitob.

### 4.3 Raqib korpusining manifesti

Gateway operatorlari endi idrok etishni sanab o'tadigan hamroh manifestini qabul qilishadi
kalibrlashdan olingan "oilalarni" xesh/ko'mish:

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

Sxema `crates/iroha_data_model/src/sorafs/moderation.rs` da yashaydi va shunday
`AdversarialCorpusManifestV1::validate()` orqali tasdiqlangan. Manifest ruxsat beradi
blokirovka qiluvchi `perceptual_family` yozuvlarini to'ldirish uchun shlyuzni rad etish ro'yxatini yuklovchi
alohida baytlar o'rniga butun deyarli takroriy klasterlar. Yugurish mumkin bo'lgan armatura
(`docs/examples/ai_moderation_perceptual_registry_202602.json`) namoyish etadi
kutilgan tartib va to'g'ridan-to'g'ri namuna shlyuzining rad etish ro'yxatiga uzatiladi.

## 5. Ijro quvuri
1. Boshqaruv DAG dan `AiModerationManifestV1` ni yuklang. Agar rad etsa
   `runner_hash` yoki `runtime_version` o'rnatilgan ikkilik faylga mos kelmaydi.
2. OCI dayjesti orqali model artefaktlarini oling, yuklashdan oldin dayjestlarni tekshiring.
3. Tarkib turi bo'yicha baholash partiyalarini tuzing; buyurtma bo'yicha saralash kerak
   Deterministik agregatsiyani ta'minlash uchun `(content_digest, manifest_id)`.
4. Har bir modelni olingan urug' bilan bajaring. Pertseptiv xeshlar uchun birlashtiring
   ansambl ko'pchilik ovoz orqali -> ball `[0,1]`.
5. Og'irlangan qisqartirilgan nisbatdan foydalangan holda ballarni `combined_score` ga jamlang:
   ```
   combined = Σ_i weight_i * clamp(score_i / threshold_i, 0, 1) / Σ_i weight_i
   ```
6. `ModerationVerdictV1` ishlab chiqaring:
   - `escalate`, agar mavjud bo'lsa, `critical_labels` yong'in yoki `combined ≥ thresholds.escalate`.
   - `quarantine`, agar `thresholds.quarantine` dan yuqori bo'lsa, lekin `escalate` dan past bo'lsa.
   - `pass` aks holda.
7. `AiModerationResultV1` davom eting va quyi oqim jarayonlarini navbatga qo'ying:
   - Karantin xizmati (agar hukm kuchaysa/karantinlar bo'lsa)
   - Shaffoflik jurnali yozuvchisi (`ModerationLedgerV1`)
   - Telemetriya eksportchisi

## 6. Kalibrlash va baholash
- **Maʼlumotlar toʻplami:** Asosiy kalibrlash siyosat bilan tuzilgan aralash korpusdan foydalanadi
  jamoani tasdiqlash. Ma'lumotnoma `calibration_dataset` da qayd etilgan.
- **Metriklar:** Hisoblash Brier balli, kutilayotgan kalibrlash xatosi (ECE) va AUROC
  har bir model va birlashtirilgan hukm. Oylik qayta kalibrlash saqlanishi kerak
  `Brier ≤ 0.18` va `ECE ≤ 0.05`. SoraFS hisobotlar daraxtida saqlangan natijalar
  (masalan, [2026 yil fevral kalibrlash](../sorafs/reports/ai-moderation-calibration-202602.md)).
- **Jadval:** Oylik qayta kalibrlash (birinchi dushanba). Favqulodda qayta kalibrlash
  agar drift yong'in haqida ogohlantirsa ruxsat beriladi.
- **Jarayon:** Kalibrlash majmuasida deterministik baholash quvurini ishga tushirish,
  `thresholds` regenerate, manifestni yangilash, boshqaruv ovozi uchun bosqich o'zgarishlari.

## 7. Qadoqlash va joylashtirish
- `docker buildx bake -f docker/ai_moderation.hcl` orqali OCI tasvirlarini yarating.
- Rasmlarga quyidagilar kiradi:
  - Qulflangan Python env (`poetry.lock`) yoki Rust ikkilik `Cargo.lock`.
  - Xeshlangan ONNX og'irliklari bilan `models/` katalogi.
  - HTTP/gRPC API-ni ochuvchi `run_moderation.py` (yoki Rust ekvivalenti) kirish nuqtasi.
- Artefaktlarni `registry.sora.net/ministry/ai-moderation/<model>@sha256:<digest>` ga e'lon qiling.
- `sorafs_ai_runner` kassasining bir qismi sifatida yuguruvchi ikkilik kemalar. Qurilish quvuri
  manifest xeshni ikkilik faylga joylashtiradi (`/v1/info` orqali ochiladi).

## 8. Telemetriya va kuzatuvchanlik
- Prometheus ko'rsatkichlari:
  - `moderation_requests_total{verdict}`
  - `moderation_model_score_bucket{model_id,label}`
  - `moderation_combined_score_bucket`
  - `moderation_inference_latency_seconds_bucket`
  - `moderation_runner_manifest_info{manifest_id, runtime_version}`
- Jurnallar: `request_id`, `manifest_id`, `verdict` va dayjestli JSON qatorlari
  saqlangan natija. Xom ballar jurnallarda ikki kasrga qisqartiriladi.
- `dashboards/grafana/ministry_moderation_overview.json` da saqlangan asboblar paneli
  (birinchi kalibrlash hisoboti bilan birga nashr etilgan).
- Ogohlantirish chegaralari:
  - etishmayotgan yutish (`moderation_requests_total` 10 daqiqa davomida to'xtab qoldi).
  - Driftni aniqlash (modelning o'rtacha ko'rsatkichi deltaning 7 kunlik o'rtacha darajasiga nisbatan >20%).
  - Noto'g'ri ijobiy kechikish (karantin navbati > 30 daqiqa davomida 50 ta element).

## 9. Boshqaruv va o'zgarishlarni nazorat qilish
- Manifestlar ikki imzo talab qiladi: Vazirlik kengashi a'zosi + moderatorlik SRE
  qo'rg'oshin. Imzolar `AiModerationManifestV1.governance_signature` da yozilgan.
- O'zgarishlar `ModerationManifestChangeProposalV1` dan Torii gacha. Xeshlar
  DAG boshqaruviga kirdi; taklif amalga oshirilgunga qadar joylashtirish bloklanadi
  kuchga kirgan.
- Runner binaries embed `runner_hash`; Agar xeshlar ajralib chiqsa, CI joylashtirishni rad etadi.
- Shaffoflik: haftalik `ModerationScorecardV1` umumiy hajmi, hukmlar aralashmasi,
  va apellyatsiya natijalari. Sora Parlament portalida chop etilgan.

## 10. Xavfsizlik va maxfiylik
- Tarkibni sindirishda BLAKE3 ishlatiladi. Xom yuklar karantindan tashqarida hech qachon saqlanib qolmaydi.
- Karantinga kirish uchun Just-In-Time ruxsatnomalari talab qilinadi; barcha kirishlar qayd etilgan.
- Runner 512 Mb xotira chegaralari va 120 soniyalarni qo'llab, ishonchsiz kontentni sinovdan o'tkazadi
  devor soati qo'riqchilari.
- Bu yerda differentsial maxfiylik qo'llanilmaydi; shlyuzlar karantin + auditga tayanadi
  o'rniga ish oqimlari. Tahrirlash siyosatlari shlyuzga muvofiqlik rejasiga amal qiladi
  (`docs/source/sorafs_gateway_compliance_plan.md`; portal nusxasi kutilmoqda).

## 11. Kalibrlash nashri (2026-02)
- **Manifest:** `docs/examples/ai_moderation_calibration_manifest_202602.json`
  boshqaruv tomonidan imzolangan `AiModerationManifestV1` (ID
  `c9bdf0b2-63a3-4a90-8d70-908d119c2c7e`), ma'lumotlar to'plamiga havola
  `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`, yuguruvchi xeshi
  `ea3c0fd0ff4bd4510e94c7c293b261f601cc0c4f9fbacd99b0401d233a7cdc20` va
  2026-02 kalibrlash chegaralari (`quarantine = 0.42`, `escalate = 0.78`).
- **Skorlar jadvali:** `docs/examples/ai_moderation_calibration_scorecard_202602.json`
  Bundan tashqari, inson tomonidan o'qiladigan hisobot
  `[SoraFS Reports › AI Moderation Calibration 2026-02](../sorafs/reports/ai-moderation-calibration-202602.md)`
  Har bir model uchun Brier, ECE, AUROC va hukmlar aralashmasini oling. Birlashtirilgan ko'rsatkichlar
  maqsadlarga erishdi (`Brier = 0.126`, `ECE = 0.034`).
- **Boshqaruv paneli va ogohlantirishlar:** `dashboards/grafana/ministry_moderation_overview.json`
  va `dashboards/alerts/ministry_moderation_rules.yml` (regressiya testlari bilan
  `dashboards/alerts/tests/ministry_moderation_rules.test.yml`) taqdim etadi
  chiqarish uchun moderatsiya ingest/kechikish/drift monitoringi hikoyasi talab qilinadi.## 12. Qayta ishlab chiqarish sxemasi va validator (MINFO-1b)
- Kanonik Norito turlari endi SoraFS sxemasining qolgan qismi bilan birga yashaydi.
  `crates/iroha_data_model/src/sorafs/moderation.rs`. The
  `ModerationReproManifestV1`/`ModerationReproBodyV1` tuzilmalari
  manifest UUID, yuguruvchi xeshi, model dayjestlari, chegara to'plami va urug'lik materiali.
  `ModerationReproManifestV1::validate` sxema versiyasini amalga oshiradi
  (`MODERATION_REPRO_MANIFEST_VERSION_V1`), har bir manifestning bajarilishini ta'minlaydi
  kamida bitta model va imzolovchi va har bir `SignatureOf<ModerationReproBodyV1>` ni tasdiqlaydi
  mashinada o'qiladigan xulosani qaytarishdan oldin.
- Operatorlar birgalikda validatorni orqali chaqirishi mumkin
  `sorafs_cli moderation validate-repro --manifest=PATH [--format=json|norito]`
  (`crates/sorafs_orchestrator/src/bin/sorafs_cli.rs` da amalga oshirilgan). CLI
  ostida chop etilgan JSON artefaktlarini qabul qiladi
  `docs/examples/ai_moderation_calibration_manifest_202602.json` yoki xom
  Norito kodlash va manifest bilan birga model/imzolar sonini chop etadi
  tekshirish muvaffaqiyatli bo'lgandan keyin vaqt tamg'asi.
- Shlyuzlar va avtomatlashtirish bir xil yordamchiga bog'lanadi, shuning uchun takrorlanish qobiliyati namoyon bo'ladi
  sxemalar o'zgarganda, dayjestlar yo'q bo'lganda yoki aniqlik bilan rad etilishi mumkin
  imzolar tekshirilmaydi.
- Raqib korpus to'plamlari bir xil naqshga amal qiladi:
  `sorafs_cli moderation validate-corpus --manifest=PATH [--format=json|norito]`
  `AdversarialCorpusManifestV1` ni tahlil qiladi, sxema versiyasini amalga oshiradi va rad etadi
  oilalar, variantlar yoki barmoq izlari metamaʼlumotlarini oʻtkazib yuboradigan manifestlar. Muvaffaqiyatli
  yugurishlar vaqt tamg'asi, kohort yorlig'i va oila/variant sonlarini chiqaradi
  shuning uchun operatorlar shlyuzni rad etish ro'yxati yozuvlarini yangilashdan oldin dalillarni mahkamlashlari mumkin
  4.3-bo'limda tavsiflangan.

## 13. Kuzatuvlarni oching
- 2026-03-02 dan keyin oylik qayta kalibrlash oynalari amalda davom etadi
  6-bo'limdagi tartib; nashr qilish `ai-moderation-calibration-<YYYYMM>.md`
  SoraFS hisobotlar daraxti ostidagi yangilangan manifest/koʻrsatkich kartalari toʻplamlari bilan bir qatorda.
- MINFO-1b va MINFO-1c (qayta ishlab chiqarish manifest validatorlari va raqiblar)
  korpus reestri) yo'l xaritasida alohida kuzatilishi mumkin.