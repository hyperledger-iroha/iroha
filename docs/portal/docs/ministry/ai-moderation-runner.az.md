---
lang: az
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

# AI Moderasiya Runner Spesifikasiyası

Bu spesifikasiya **MINFO-1 — Süni intellekt yaradılmasının sənədləşdirmə hissəsini yerinə yetirir
moderasiya bazası**. üçün deterministik icra müqaviləsini müəyyən edir
İnformasiya Nazirliyinin moderasiya xidməti beləliklə hər bir şlüz eyni işləyə bilər
müraciətlər və şəffaflıq axınlarından əvvəl boru kəmərləri (SFM-4/SFM-4b). Bütün davranış
burada təsvir edilənlər açıq şəkildə məlumat xarakteri daşımadığı halda normativdir.

## 1. Məqsədlər və əhatə dairəsi
- Gateway məzmununu qiymətləndirən təkrarlana bilən moderasiya komitəsi təmin edin
  (obyektlər, manifestlər, metadata, audio) heterojen modellərdən istifadə etməklə.
- Operatorlar arasında deterministik icraya zəmanət verin: sabit opset, toxumlanmış
  tokenizasiya, məhdud dəqiqlik və versiyalı artefaktlar.
- Auditə hazır artefaktlar hazırlayın: manifestlər, bal kartları, kalibrləmə sübutları,
  və idarəetmə DAG-da dərc olunmaq üçün uyğun olan şəffaflıq həzmləri.
- Səthi telemetriya beləliklə SRE-lər sürüşməni, yanlış pozitivləri və dayanma müddətini aşkar edə bilsin
  xam istifadəçi məlumatlarını toplamadan.

## 2. Deterministik İcra Müqaviləsi
- **Runtime:** ONNX Runtime 1.19.x (CPU backend) AVX2 deaktiv edilmiş və tərtib edilmişdir
  Əməliyyat kodu dəstini sabit saxlamaq üçün `--enable-extended-minimal-build`. CUDA/Metal
  istehsal müddətinə açıq şəkildə icazə verilmir.
- **Opset:** `opset=17`. Daha yeni opsetləri hədəfləyən modellər aşağıya çevrilməlidir
  və qəbuldan əvvəl təsdiq edilmişdir.
- **Toxum törəməsi:** Hər bir qiymətləndirmə RNG toxumundan əldə edilir
  `BLAKE3(content_digest || manifest_id || run_nonce)` harada `run_nonce` gəlir
  idarəetmə tərəfindən təsdiqlənmiş manifestdən. Toxumlar bütün stokastik komponentləri qidalandırır
  (şüa axtarışı, buraxılış keçidləri) beləliklə, nəticələr bit-bit təkrarlana bilər.
- **Threading:** Hər modelə bir işçi. Paralellik qaçışçı tərəfindən əlaqələndirilir
  ortaq dövlət yarış şərtlərindən qaçmaq üçün orkestr. BLAS kitabxanaları fəaliyyət göstərir
  tək yivli rejim.
- **Rəqəmlər:** FP16 yığılması qadağandır. FP32 ara məhsullarından istifadə edin və sıxın
  aqreqasiyadan əvvəl dörd onluq yerlərə çıxır.

## 3. Komitənin tərkibi
Əsas komitə üç model ailədən ibarətdir. İdarəetmə əlavə edə bilər
modellər, lakin minimum kvorum razı qalmalıdır.

| Ailə | Əsas Model | Məqsəd |
|--------|----------------|---------|
| Vizyon | OpenCLIP ViT-H/14 (təhlükəsizliyin dəqiq tənzimlənməsi) | Vizual qaçaqmalçılıq, zorakılıq, CSAM göstəricilərini aşkar edir. |
| Multimodal | LLaVA-1.6 34B Təhlükəsizlik | Mətn + şəkil qarşılıqlı əlaqəsini, kontekstli işarələri, təcavüzü çəkir. |
| Perceptual | pHash + aHash + NeuralHash-lite ansamblı | Bilinən pis materialın dublikatına yaxın aşkarlanması və geri çağırılması. |

Hər bir model girişi müəyyən edir:
- `model_id` (UUID)
- `artifact_digest` (OCI şəklinin BLAKE3-256)
- `weights_digest` (ONNX-in BLAKE3-256 və ya birləşdirilmiş qoruyucu blok)
- `opset` (`17`-ə bərabər olmalıdır)
- `weight` (komitə çəkisi, standart `1.0`)
- `critical_labels` (dərhal `Escalate`-i işə salan etiketlər dəsti)
- `max_eval_ms` (deterministik nəzarətçi itlər üçün qoruyucu barmaqlıq)

## 4. Norito Manifestlər və Nəticələr

### 4.1 Komitənin Manifesti
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

### 4.2 Qiymətləndirmə Nəticəsi
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

Qaçışçı deterministik `AiModerationDigestV1` (BLAKE3) yaymalıdır.
şəffaflıq qeydləri üçün seriallaşdırılmış nəticə) və nəticələri moderasiyaya əlavə edin
hökm `pass` olmadıqda kitab.

### 4.3 Düşmən Korpus Manifest

Gateway operatorları indi qavrayışı sadalayan yoldaş manifestini qəbul edirlər
Kalibrləmə əməliyyatlarından əldə edilən hash/yerləşdirmə “ailələri”:

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

Sxem `crates/iroha_data_model/src/sorafs/moderation.rs`-də yaşayır və belədir
`AdversarialCorpusManifestV1::validate()` vasitəsilə təsdiq edilmişdir. Manifest imkan verir
bloklayan `perceptual_family` girişlərini doldurmaq üçün şlüzdən imtina siyahısı yükləyicisi
fərdi baytların əvəzinə bütövlükdə təkrarlanan klasterlər. İşlənə bilən qurğu
(`docs/examples/ai_moderation_perceptual_registry_202602.json`) nümayiş etdirir
gözlənilən layout və birbaşa nümunə gateway inkar siyahısına qidalanır.

## 5. İcra Boru Kəməri
1. İdarəetmə DAG-dan `AiModerationManifestV1` yükləyin. Əgər rədd et
   `runner_hash` və ya `runtime_version` yerləşdirilmiş ikiliyə uyğun gəlmir.
2. Yükləmədən əvvəl həzmləri yoxlayaraq OCI həzm sistemi vasitəsilə model artefaktlarını əldə edin.
3. Məzmun növü üzrə qiymətləndirmə qruplarını qurun; sifariş üzrə çeşidlənməlidir
   Deterministik birləşməni təmin etmək üçün `(content_digest, manifest_id)`.
4. Hər bir modeli əldə edilmiş toxumla icra edin. Qavrama hashləri üçün birləşdirin
   ansambl səs çoxluğu ilə -> `[0,1]`-də hesab.
5. Çəkili kəsilmiş nisbətdən istifadə edərək xalları `combined_score`-ə birləşdirin:
   ```
   combined = Σ_i weight_i * clamp(score_i / threshold_i, 0, 1) / Σ_i weight_i
   ```
6. `ModerationVerdictV1` istehsal edin:
   - `escalate`, əgər varsa, `critical_labels` yanğın və ya `combined ≥ thresholds.escalate`.
   - `quarantine`, əgər `thresholds.quarantine`-dən yuxarı, lakin `escalate`-dən aşağıdırsa.
   - `pass` əks halda.
7. `AiModerationResultV1`-ə davam edin və aşağı axın proseslərini sıralayın:
   - Karantin xidməti (hökm artarsa/karantin olarsa)
   - Şəffaflıq jurnalı yazıcısı (`ModerationLedgerV1`)
   - Telemetriya ixracatçısı

## 6. Kalibrləmə və Qiymətləndirmə
- **Datasets:** İlkin kalibrləmə siyasətlə seçilmiş qarışıq korpusdan istifadə edir
  komandanın təsdiqi. İstinad `calibration_dataset`-də qeydə alınıb.
- **Metriklər:** Hesablama Brier balı, Gözlənilən Kalibrləmə Xətası (ECE) və AUROC
  model və birləşmiş hökmə görə. Aylıq yenidən kalibrləmə SAXLANMALIDIR
  `Brier ≤ 0.18` və `ECE ≤ 0.05`. SoraFS hesabat ağacında saxlanılan nəticələr
  (məsələn, [Fevral 2026-cı il kalibrləmə](../sorafs/reports/ai-moderation-calibration-202602.md)).
- **Cədvəl:** Aylıq yenidən kalibrləmə (ilk bazar ertəsi). Fövqəladə yenidən kalibrləmə
  sürüşmə yanğın xəbərdarlığı olduqda icazə verilir.
- **Proses:** Kalibrləmə dəstində deterministik qiymətləndirmə boru kəmərini işə salın,
  `thresholds` bərpa edin, manifesti yeniləyin, idarəetmə səsverməsi üçün dəyişiklikləri et.

## 7. Qablaşdırma və Yerləşdirmə
- `docker buildx bake -f docker/ai_moderation.hcl` vasitəsilə OCI şəkillərini yaradın.
- Şəkillərə daxildir:
  - Kilidlənmiş Python env (`poetry.lock`) və ya Rust binar `Cargo.lock`.
  - Hashed ONNX çəkiləri ilə `models/` kataloqu.
  - HTTP/gRPC API-ni ifşa edən `run_moderation.py` (və ya Rust ekvivalenti) giriş nöqtəsi.
- Artefaktları `registry.sora.net/ministry/ai-moderation/<model>@sha256:<digest>`-də dərc edin.
- `sorafs_ai_runner` qutusunun bir hissəsi kimi qaçışçı ikili gəmilər. Tikinti boru kəməri
  manifest hash-i binar sistemə daxil edir (`/v2/info` vasitəsilə ifşa olunur).

## 8. Telemetriya və Müşahidə Edilə bilənlik
- Prometheus ölçüləri:
  - `moderation_requests_total{verdict}`
  - `moderation_model_score_bucket{model_id,label}`
  - `moderation_combined_score_bucket`
  - `moderation_inference_latency_seconds_bucket`
  - `moderation_runner_manifest_info{manifest_id, runtime_version}`
- Qeydlər: `request_id`, `manifest_id`, `verdict` və həzm ilə JSON xətləri
  saxlanılan nəticə. Xam xallar jurnallarda iki onluq yerlərə düzəldilir.
- `dashboards/grafana/ministry_moderation_overview.json`-də saxlanılan idarə panelləri
  (ilk kalibrləmə hesabatı ilə birlikdə dərc edilmişdir).
- Xəbərdarlıq hədləri:
  - Çatışmayan qəbul (`moderation_requests_total` 10 dəqiqə dayandı).
  - Drift aşkarlanması (ortalama model balı delta >20% ilə müqayisədə 7 günlük ortalama).
  - Yanlış-müsbət gecikmə (karantin növbəsi > 30 dəqiqə ərzində > 50 element).

## 9. İdarəetmə və Dəyişikliklərə Nəzarət
- Manifestlər ikili imza tələb edir: Nazirlik şurasının üzvü + moderator SRE
  aparıcı. İmzalar `AiModerationManifestV1.governance_signature`-də qeydə alınıb.
- Dəyişikliklər `ModerationManifestChangeProposalV1`-dən Torii-ə qədər davam edir. Haşlar
  DAG idarəçiliyinə daxil oldu; təklif edilənə qədər yerləşdirmə bloklanır
  qüvvəyə minmişdir.
- `runner_hash` yerləşdirən Runner binaries; Heshlər fərqli olarsa, CI yerləşdirmədən imtina edir.
- Şəffaflıq: həftəlik `ModerationScorecardV1` ümumiləşdirmə həcmi, hökmlər qarışığı,
  və şikayətin nəticələri. Sora Parlament portalında dərc olunub.

## 10. Təhlükəsizlik və Məxfilik
- Məzmun həzmləri BLAKE3 istifadə edir. Xam yüklər heç vaxt karantindən kənarda qalmır.
- Karantinə giriş Just-In-Time təsdiqini tələb edir; bütün girişlər qeyd edildi.
- Runner 512 MiB yaddaş limitini və 120 saniyəni tətbiq edərək etibarsız məzmunu qum qutusuna çevirir.
  divar saatı mühafizəçiləri.
- Burada diferensial məxfilik tətbiq olunmur; şlüzlər karantin + auditə əsaslanır
  əvəzinə iş axınları. Redaksiya siyasətləri şlüz uyğunluq planına uyğundur
  (`docs/source/sorafs_gateway_compliance_plan.md`; portalın surəti gözlənilir).

## 11. Kalibrləmə nəşri (2026-02)
- **Manifest:** `docs/examples/ai_moderation_calibration_manifest_202602.json`
  idarəetmə tərəfindən imzalanmış `AiModerationManifestV1` (ID
  `c9bdf0b2-63a3-4a90-8d70-908d119c2c7e`), verilənlər bazası arayışı
  `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`, qaçışçı hash
  `ea3c0fd0ff4bd4510e94c7c293b261f601cc0c4f9fbacd99b0401d233a7cdc20` və
  2026-02 kalibrləmə hədləri (`quarantine = 0.42`, `escalate = 0.78`).
- **Skorboard:** `docs/examples/ai_moderation_calibration_scorecard_202602.json`
  üstəgəl insan tərəfindən oxuna bilən hesabat
  `[SoraFS Reports › AI Moderation Calibration 2026-02](../sorafs/reports/ai-moderation-calibration-202602.md)`
  hər model üçün Brier, ECE, AUROC və hökm qarışığını əldə edin. Birləşdirilmiş ölçülər
  hədəflərə çatdı (`Brier = 0.126`, `ECE = 0.034`).
- **İdarəetmə panelləri və xəbərdarlıqlar:** `dashboards/grafana/ministry_moderation_overview.json`
  və `dashboards/alerts/ministry_moderation_rules.yml` (reqressiya testləri ilə
  `dashboards/alerts/tests/ministry_moderation_rules.test.yml`) təmin edir
  Yayım üçün tələb olunan moderasiya qəbulu/gecikmə/drift monitorinq hekayəsi.## 12. Təkrarlanma sxemi və validator (MINFO-1b)
- Kanonik Norito növləri indi SoraFS sxeminin qalan hissəsi ilə birlikdə yaşayır.
  `crates/iroha_data_model/src/sorafs/moderation.rs`. The
  `ModerationReproManifestV1`/`ModerationReproBodyV1` strukturları
  manifest UUID, qaçışçı hash, model həzmləri, eşik dəsti və toxum materialı.
  `ModerationReproManifestV1::validate` sxem versiyasını tətbiq edir
  (`MODERATION_REPRO_MANIFEST_VERSION_V1`), hər bir manifestin daşınmasını təmin edir
  ən azı bir model və imzalayan və hər `SignatureOf<ModerationReproBodyV1>`-i yoxlayır
  maşın tərəfindən oxuna bilən xülasə qaytarılmadan əvvəl.
- Operatorlar vasitəsilə paylaşılan validatoru çağıra bilərlər
  `sorafs_cli moderation validate-repro --manifest=PATH [--format=json|norito]`
  (`crates/sorafs_orchestrator/src/bin/sorafs_cli.rs`-də həyata keçirilir). CLI
  altında dərc olunan JSON artefaktlarını qəbul edir
  `docs/examples/ai_moderation_calibration_manifest_202602.json` və ya xam
  Norito kodlaması və manifestlə yanaşı model/imza sayılarını çap edir
  doğrulama uğurlu olduqdan sonra vaxt damğası.
- Şlüzlər və avtomatlaşdırma eyni köməkçiyə qoşulur, beləliklə təkrar istehsal olunur
  sxemlər sürüşərkən, həzmlər əskik olduqda və ya deterministik şəkildə rədd edilə bilər
  imzalar yoxlanılmır.
- Rəqib korpus paketləri eyni nümunəyə uyğundur:
  `sorafs_cli moderation validate-corpus --manifest=PATH [--format=json|norito]`
  `AdversarialCorpusManifestV1` təhlil edir, sxem versiyasını tətbiq edir və imtina edir
  ailələri, variantları və ya barmaq izi metadatasını buraxan təzahürlər. Uğurlu
  qaçışlar buraxılmış vaxt damğasını, kohort etiketini və ailə/variant sayılarını yayır
  beləliklə, operatorlar şlüzdən imtina siyahısı qeydlərini yeniləməzdən əvvəl dəlilləri təyin edə bilərlər
  Bölmə 4.3-də təsvir edilmişdir.

## 13. İzləmələri açın
- 03-02-2026 tarixindən sonra aylıq yenidən kalibrləmə pəncərələri izləməyə davam edir
  6-cı Bölmədəki prosedur; nəşr `ai-moderation-calibration-<YYYYMM>.md`
  SoraFS hesabat ağacının altında yenilənmiş manifest/bal kartı paketləri ilə yanaşı.
- MINFO-1b və MINFO-1c (reproduktivlik manifest təsdiqləyiciləri və rəqiblər)
  korpus reyestri) yol xəritəsində ayrıca izlənilir.