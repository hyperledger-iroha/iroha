---
lang: mn
direction: ltr
source: docs/portal/docs/ministry/ai-moderation-runner.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: AI Moderation Runner Specification
summary: Deterministic moderation committee design for the Ministry of Information (MINFO-1) deliverable.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# AI Moderation Runner техникийн үзүүлэлт

Энэхүү тодорхойлолт нь **MINFO-1 — AI үүсгэн байгуулах баримт бичгийн хэсгийг хангаж байна
зохицуулалтын суурь үзүүлэлт**. Энэ нь тодорхойлогч гүйцэтгэлийн гэрээг тодорхойлдог
Мэдээллийн яамны зохицуулалтын үйлчилгээ тул гарц бүр адилхан ажиллах боломжтой
давж заалдах болон ил тод байдлын урсгалын өмнө дамжуулах хоолой (SFM-4/SFM-4b). Бүх зан байдал
Мэдээллийн чанартай гэж тодорхой заагаагүй бол энд тайлбарласан нь норматив болно.

## 1. Зорилго ба хамрах хүрээ
- Гарцын агуулгыг үнэлдэг, давтагдах боломжтой зохицуулах хороогоор хангана
  (объект, манифест, мета өгөгдөл, аудио) янз бүрийн загваруудыг ашиглан.
- Операторуудын хооронд детерминистик гүйцэтгэлийг баталгаажуулах: тогтмол опсет, үржүүлсэн
  токенизаци, хязгаарлагдмал нарийвчлал, хувилбарт олдворууд.
- Аудит хийхэд бэлэн олдворуудыг гаргах: манифест, онооны хуудас, шалгалт тохируулгын нотлох баримт,
  засаглалын DAG-д нийтлэхэд тохиромжтой ил тод байдлын тойм.
- Гадаргуугийн телеметрийн тусламжтайгаар SRE нь шилжилт хөдөлгөөн, худал эерэг болон зогсолтыг илрүүлэх боломжтой
  хэрэглэгчийн түүхий мэдээллийг цуглуулахгүйгээр.

## 2. Детерминист гүйцэтгэлийн гэрээ
- **Ажиллах хугацаа:** AVX2-г идэвхгүй болгосон, эмхэтгэсэн ONNX Runtime 1.19.x (CPU backend)
  Опкодын багцыг тогтмол байлгахын тулд `--enable-extended-minimal-build`. CUDA/металл
  үйлдвэрлэлд ажиллах цагийг шууд хориглодог.
- **Опсет:** `opset=17`. Шинэ опсетуудад чиглэсэн загваруудыг доош хөрвүүлсэн байх ёстой
  элсэлтийн өмнө баталгаажуулсан.
- **Үрийн гарал үүсэл:** Үнэлгээ бүр нь RNG үрийг авдаг
  `BLAKE3(content_digest || manifest_id || run_nonce)` хаана `run_nonce` ирдэг
  засаглалын баталсан манифестаас. Үр нь бүх стохастик бүрэлдэхүүн хэсгүүдийг тэжээдэг
  (цацрагт хайлт, завсарлага сэлгэх) тул үр дүн нь битээр дахин давтагдах боломжтой.
- ** Threading:** Нэг загварт нэг ажилчин. Зэрэгцээ байдлыг гүйгч зохицуулдаг
  хамтын муж улсын уралдааны нөхцлөөс зайлсхийхийн тулд найруулагч. BLAS номын сангууд ажилладаг
  нэг урсгалтай горим.
- **Тоонууд:** FP16 хуримтлуулахыг хориглоно. FP32 завсрын бүтээгдэхүүн ба хавчаарыг ашиглана
  нэгтгэхээс өмнө аравтын дөрвөн орон руу гарна.

## 3. Хорооны бүрэлдэхүүн
Суурь хороонд гурван загвар гэр бүл багтдаг. Засаглал нэмж болно
загварууд, гэхдээ хамгийн бага чуулга хангагдсан хэвээр байх ёстой.

| Гэр бүл | Үндсэн загвар | Зорилго |
|--------|----------------|---------|
| Алсын хараа | OpenCLIP ViT-H/14 (аюулгүй байдлыг нарийн тохируулсан) | Харааны хууль бус бараа, хүчирхийлэл, CSAM үзүүлэлтүүдийг илрүүлдэг. |
| Multimodal | LLaVA-1.6 34B Аюулгүй байдал | Текст + зургийн харилцан үйлчлэл, контекстийн дохиолол, дарамтыг авдаг. |
| Мэдрэхүйн | pHash + aHash + NeuralHash-lite чуулга | Мэдэгдэж буй муу материалыг хурдан шуурхай илрүүлэх, эргүүлэн татах. |

Загварын оруулга бүр дараахь зүйлийг заана.
- `model_id` (UUID)
- `artifact_digest` (OCI зургийн BLAKE3-256)
- `weights_digest` (ONNX-ийн BLAKE3-256 эсвэл нэгдсэн хамгаалалттай блок)
- `opset` (`17`-тэй тэнцүү байх ёстой)
- `weight` (хорооны жин, анхдагч `1.0`)
- `critical_labels` (`Escalate`-г шууд идэвхжүүлдэг шошгоны багц)
- `max_eval_ms` (детерминист харуулын хашлага)

## 4. Norito Манифест ба Үр дүн

### 4.1 Хорооны манифест
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

### 4.2 Үнэлгээний үр дүн
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

Гүйгч нь тодорхойлогч `AiModerationDigestV1` (BLAKE3) ялгаруулах ЗААВАЛ
цувралын үр дүн) ил тод байдлын бүртгэлд зориулж үр дүнг тохируулгад хавсаргана
Шүүхийн шийдвэр `pass` биш бол дэвтэр.

### 4.3 Эсэргүүцлийн корпусын манифест

Гарцын операторууд одоо перцепцийг тоочдог хамтрагч манифестийг залгиж байна
Шалгалт тохируулгын үр дүнд бий болсон "гэр бүл"-ийг хэш/суулгах:

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

Уг схем нь `crates/iroha_data_model/src/sorafs/moderation.rs`-д амьдардаг бөгөөд ийм байна
`AdversarialCorpusManifestV1::validate()`-ээр баталгаажуулсан. Манифест нь үүнийг зөвшөөрдөг
блоклосон `perceptual_family` оруулгуудыг бөглөх гарц үгүйсгэх жагсаалт дуудагч
бие даасан байтуудын оронд бараг давхардсан кластерууд. Ажиллах боломжтой бэхэлгээ
(`docs/examples/ai_moderation_perceptual_registry_202602.json`) харуулж байна
хүлээгдэж буй байршил болон шууд дээж гарц үгүйсгэх жагсаалт руу тэжээгддэг.

## 5. Гүйцэтгэлийн шугам хоолой
1. Засаглалын DAG-аас `AiModerationManifestV1` ачаална. Хэрэв татгалзах
   `runner_hash` эсвэл `runtime_version` суулгасан хоёртын файлтай таарахгүй байна.
2. Загварын олдворуудыг ачаалахаас өмнө баталгаажуулж, OCI дижестээр дамжуулан татаж аваарай.
3. Үнэлгээний багцыг агуулгын төрлөөр бүрдүүлэх; захиалгаар эрэмбэлэх ёстой
   `(content_digest, manifest_id)` нь тодорхойлогдсон нэгтгэлийг баталгаажуулна.
4. Загвар бүрийг гарган авсан үрээр гүйцэтгэнэ. Ойлгомжтой хэшүүдийн хувьд нэгтгэнэ үү
   олонхийн саналаар чуулга -> оноо `[0,1]`.
5. Жинлэсэн хасагдсан харьцааг ашиглан оноог `combined_score` болгон нэгтгэнэ үү:
   ```
   combined = Σ_i weight_i * clamp(score_i / threshold_i, 0, 1) / Σ_i weight_i
   ```
6. `ModerationVerdictV1` үйлдвэрлэх:
   - `escalate` хэрэв байгаа бол `critical_labels` эсвэл `combined ≥ thresholds.escalate`.
   - `quarantine` `thresholds.quarantine`-ээс дээш, гэхдээ `escalate`-ээс доош байвал.
   - `pass` өөрөөр.
7. `AiModerationResultV1`-г үргэлжлүүлж, урсгалын доод процессуудыг дараалалд оруулаарай:
   - Хорио цээрийн үйлчилгээ (хэрэв шүүхийн шийдвэр даамжирвал/хорио цээрийн дэглэм тогтоовол)
   - Ил тод байдлын бүртгэл бичигч (`ModerationLedgerV1`)
   - Телеметрийн экспортлогч

## 6. Тохируулга & Үнэлгээ
- **Өгөгдлийн багц:** Суурь шалгалт тохируулга нь бодлогод тохируулсан холимог корпусыг ашигладаг
  багийн зөвшөөрөл. Лавлагаа `calibration_dataset`-д бүртгэгдсэн.
- **Хэмжээ:** Тооцоолох Бриерийн оноо, Хүлээгдэж буй шалгалт тохируулгын алдаа (ECE) болон AUROC
  загвар тус бүр болон хосолсон дүгнэлт. Сар бүр дахин тохируулга хийх ёстой
  `Brier ≤ 0.18` болон `ECE ≤ 0.05`. SoraFS тайлангийн модонд хадгалагдсан үр дүн
  (жишээ нь, [2026 оны 2-р сарын шалгалт тохируулга](../sorafs/reports/ai-moderation-calibration-202602.md)).
- **Хуваарь:** Сар бүрийн дахин тохируулга (эхний даваа). Яаралтай дахин тохируулга хийх
  Хэрэв дрейф гал түймрийн дохио өгвөл зөвшөөрнө.
- **Процесс:** Шалгалт тохируулгын багц дээр тодорхойлогч үнэлгээний дамжуулах хоолойг ажиллуулах,
  `thresholds` дахин үүсгэх, манифестыг шинэчлэх, засаглалын санал хураалтад өөрчлөлт оруулах үе шат.

## 7. Савлах, байршуулах
- `docker buildx bake -f docker/ai_moderation.hcl`-ээр дамжуулан OCI зургийг бүтээх.
- Зурганд:
  - Түгжигдсэн Python env (`poetry.lock`) эсвэл Rust хоёртын `Cargo.lock`.
  - Хашлагдсан ONNX жин бүхий `models/` лавлах.
  - `run_moderation.py` (эсвэл зэвтэй тэнцэх) нэвтрэх цэг нь HTTP/gRPC API-г харуулж байна.
- `registry.sora.net/ministry/ai-moderation/<model>@sha256:<digest>`-д олдворуудыг нийтлэх.
- `sorafs_ai_runner` хайрцагны нэг хэсэг болох гүйгч хоёртын хөлөг онгоц. Барилгын шугам хоолой
  манифест хэшийг хоёртын системд оруулдаг (`/v1/info`-ээр дамжуулан ил гарсан).

## 8. Телеметр ба ажиглалт
- Prometheus хэмжүүр:
  - `moderation_requests_total{verdict}`
  - `moderation_model_score_bucket{model_id,label}`
  - `moderation_combined_score_bucket`
  - `moderation_inference_latency_seconds_bucket`
  - `moderation_runner_manifest_info{manifest_id, runtime_version}`
- Бүртгэлүүд: `request_id`, `manifest_id`, `verdict` бүхий JSON мөрүүд болон дижест
  хадгалагдсан үр дүнгийн. Түүхий оноог логонд аравтын хоёр орон хүртэл бууруулна.
- `dashboards/grafana/ministry_moderation_overview.json`-д хадгалагдсан хяналтын самбар
  (эхний шалгалт тохируулгын тайлангийн хамт нийтлэгдсэн).
- Анхааруулгын босго:
  - Залгих дутуу (`moderation_requests_total` 10 минутын турш зогссон).
  - Дрифт илрүүлэх (загварын дундаж оноо нь 7 өдрийн дундажтай харьцуулахад дельта >20%).
  - Хуурамч эерэг хоцрогдол (хорио цээрийн дараалал > 50 зүйл > 30 минут).

## 9. Засаглал ба өөрчлөлтийн хяналт
- Манифестэд давхар гарын үсэг шаардлагатай: Яамны зөвлөлийн гишүүн + зохицуулагч SRE
  хар тугалга. `AiModerationManifestV1.governance_signature`-д бүртгэгдсэн гарын үсэг.
- `ModerationManifestChangeProposalV1`-ээс Torii хүртэлх өөрчлөлтүүд. Хэш
  засаглалын DAG-д орсон; санал гартал байршуулалтыг блоклосон
  хуульчилсан.
- Runner хоёртын файлыг `runner_hash` оруулах; Хэшүүд зөрүүтэй байвал CI нь байршуулахаас татгалздаг.
- Ил тод байдал: долоо хоног бүр `ModerationScorecardV1` хураангуй хэмжээ, шийдвэрийн холимог,
  болон давж заалдах үр дүн. Сора Парламентын порталд нийтэлсэн.

## 10. Аюулгүй байдал ба нууцлал
- Агуулга нь BLAKE3-ийг ашигладаг. Түүхий ачаа нь хорио цээрийн гадуур хэзээ ч үлддэг.
- Хорио цээрийн дэглэмд нэвтрэхийн тулд цаг тухайд нь зөвшөөрөл авах шаардлагатай; бүх хандалтыг бүртгэсэн.
- Runner нь найдвартай бус контентыг хамгаалж, 512 МБ санах ойн хязгаар болон 120 секундын багтаамжтай.
  ханын цагны хамгаалагч.
- Энд ялгавартай нууцлалыг ашиглахгүй; гарц нь хорио цээрийн + аудит дээр тулгуурладаг
  оронд нь ажлын урсгал. Засварлах бодлого нь гарцыг дагаж мөрдөх төлөвлөгөөг дагаж мөрддөг
  (`docs/source/sorafs_gateway_compliance_plan.md`; портал хуулбар хүлээгдэж байна).

## 11. Шалгалт тохируулгын хэвлэл (2026-02)
- **Манифест:** `docs/examples/ai_moderation_calibration_manifest_202602.json`
  засаглалын гарын үсэгтэй `AiModerationManifestV1` (ID
  `c9bdf0b2-63a3-4a90-8d70-908d119c2c7e`), өгөгдлийн багцын лавлагаа
  `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`, гүйгч хэш
  `ea3c0fd0ff4bd4510e94c7c293b261f601cc0c4f9fbacd99b0401d233a7cdc20`, мөн
  2026-02 оны шалгалт тохируулгын босго (`quarantine = 0.42`, `escalate = 0.78`).
- **Онооны самбар:** `docs/examples/ai_moderation_calibration_scorecard_202602.json`
  дээр нь хүний унших боломжтой тайлан
  `[SoraFS Reports › AI Moderation Calibration 2026-02](../sorafs/reports/ai-moderation-calibration-202602.md)`
  загвар бүрийн Brier, ECE, AUROC болон шийдвэрийн холимогийг авах. Хосолсон хэмжүүрүүд
  зорилтот түвшинд хүрсэн (`Brier = 0.126`, `ECE = 0.034`).
- **Хяналтын самбар ба анхааруулга:** `dashboards/grafana/ministry_moderation_overview.json`
  болон `dashboards/alerts/ministry_moderation_rules.yml` (регрессийн тесттэй
  `dashboards/alerts/tests/ministry_moderation_rules.test.yml`) хангана
  Дамжуулахад шаардлагатай залгих/хоцролт/дрифт хянах түүх.## 12. Дахин үржихүйн схем ба баталгаажуулагч (MINFO-1b)
- Каноник Norito төрлүүд одоо SoraFS схемийн бусадтай зэрэгцэн ажиллаж байна.
  `crates/iroha_data_model/src/sorafs/moderation.rs`. The
  `ModerationReproManifestV1`/`ModerationReproBodyV1` бүтэц нь
  манифест UUID, гүйгч хэш, загвар дижест, босго багц, үрийн материал.
  `ModerationReproManifestV1::validate` нь схемийн хувилбарыг хэрэгжүүлдэг
  (`MODERATION_REPRO_MANIFEST_VERSION_V1`) нь манифест бүрийг цаг хугацаатай байлгахыг баталгаажуулдаг
  хамгийн багадаа нэг загвар болон гарын үсэг зурсан бөгөөд `SignatureOf<ModerationReproBodyV1>` бүрийг баталгаажуулна
  машинаар уншигдахуйц хураангуйг буцаахаас өмнө.
- Операторууд хуваалцсан баталгаажуулагчийг дамжуулан дуудаж болно
  `sorafs_cli moderation validate-repro --manifest=PATH [--format=json|norito]`
  (`crates/sorafs_orchestrator/src/bin/sorafs_cli.rs`-д хэрэгжүүлсэн). CLI
  доор нийтлэгдсэн JSON олдворуудыг хүлээн зөвшөөрдөг
  `docs/examples/ai_moderation_calibration_manifest_202602.json` эсвэл түүхий
  Norito кодчилол нь манифестын хажууд загвар/гарын үсгийн тоог хэвлэдэг.
  Баталгаажуулалт амжилттай болсны дараа цагийн тэмдэг.
- Гарцууд болон автоматжуулалт нь ижил туслагчтай холбогддог тул дахин давтагдах чадвар илэрдэг
  Схемүүд шилжих, дижест байхгүй, эсвэл байхгүй үед тодорхой хэмжээгээр татгалзаж болно
  гарын үсгийг баталгаажуулж чадаагүй.
- Өрсөлдөгч корпусын багцууд нь ижил хэв маягийг дагадаг:
  `sorafs_cli moderation validate-corpus --manifest=PATH [--format=json|norito]`
  `AdversarialCorpusManifestV1`-г задлан, схемийн хувилбарыг хэрэгжүүлж, татгалздаг
  гэр бүл, хувилбарууд эсвэл хурууны хээний мета өгөгдлийг орхисон илрэл. Амжилттай
  гүйлтүүд нь гаргасан цагийн тэмдэг, когортын шошго, гэр бүл/хувилбарын тоог ялгаруулдаг
  Тиймээс операторууд гарцыг үгүйсгэх жагсаалтын оруулгуудыг шинэчлэхээс өмнө нотлох баримтыг тогтоох боломжтой
  4.3-т тодорхойлсон.

## 13. Нээлттэй дагаж мөрдөх
- 2026-03-02-ны өдрөөс хойш сар бүр дахин шалгалт тохируулга хийх цонхнууд үргэлжилсээр байна
  6-р хэсэгт заасан журам; `ai-moderation-calibration-<YYYYMM>.md` нийтлэх
  SoraFS тайлангийн модны доор шинэчлэгдсэн манифест/онолын картын багцын хамт.
- MINFO-1b ба MINFO-1c (дахин давтагдах чадварын манифест баталгаажуулагч ба өрсөлдөгчид)
  корпусын бүртгэл) замын зураглалд тусад нь дагаж мөрддөг.