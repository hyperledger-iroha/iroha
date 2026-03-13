---
lang: ka
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

# AI Moderation Runner სპეციფიკაცია

ეს სპეციფიკაცია აკმაყოფილებს **MINFO-1-ის დოკუმენტაციის ნაწილს — დაადგინეთ AI
ზომიერება საბაზისო **. იგი განსაზღვრავს დეტერმინისტულ აღსრულების ხელშეკრულებას
ინფორმაციის სამინისტროს მოდერაციის სერვისი, რათა ყველა კარიბჭე იდენტური იყოს
მილსადენები გასაჩივრებამდე და გამჭვირვალობის ნაკადები (SFM-4/SFM-4b). ყველა ქცევა
აქ აღწერილი ნორმატიულია, თუ ცალსახად არ არის მონიშნული, როგორც ინფორმაციული.

## 1. მიზნები და სფერო
- უზრუნველყოთ რეპროდუცირებადი მოდერაციის კომიტეტი, რომელიც აფასებს კარიბჭის შინაარსს
  (ობიექტები, მანიფესტები, მეტამონაცემები, აუდიო) ჰეტეროგენული მოდელების გამოყენებით.
- გარანტია დეტერმინისტული შესრულების ოპერატორებს შორის: ფიქსირებული opset, seeded
  ტოკენიზაცია, შეზღუდული სიზუსტე და ვერსიული არტეფაქტები.
- შექმენით აუდიტისთვის მზა არტეფაქტები: მანიფესტები, ქულების ბარათები, კალიბრაციის მტკიცებულებები,
  და გამჭვირვალობის დაიჯესტები, რომლებიც შესაფერისია მმართველობით DAG-ში გამოსაქვეყნებლად.
- ზედაპირის ტელემეტრია, რათა SRE-ებმა შეძლონ დრეიფის, ცრუ დადებითი და შეფერხების ამოცნობა
  მომხმარებლის ნედლეული მონაცემების შეგროვების გარეშე.

## 2. დეტერმინისტული აღსრულების ხელშეკრულება
- **გაშვების დრო:** ONNX Runtime 1.19.x (CPU backend) შედგენილი AVX2 გამორთულია და
  `--enable-extended-minimal-build` ოპკოდის ნაკრების დაფიქსირების მიზნით. კუდა/მეტალი
  გაშვების დრო აშკარად აკრძალულია წარმოებაში.
- **ოპსეტი:** `opset=17`. მოდელები, რომლებიც მიზნად ისახავს უახლეს ოპსეტებს, უნდა იყოს გადაყვანილი
  და დამოწმებული მიღებამდე.
- ** თესლის წარმოშობა:** ყოველი შეფასება გამომდინარეობს RNG თესლიდან
  `BLAKE3(content_digest || manifest_id || run_nonce)` სადაც მოდის `run_nonce`
  მმართველობის მიერ დამტკიცებული მანიფესტიდან. თესლი კვებავს ყველა სტოქასტურ კომპონენტს
  (სხივის ძებნა, გამოშვების გადართვა) ასე რომ, შედეგები ბიტ-ბიტი რეპროდუცირებადია.
- ** ძაფები: ** ერთი მუშა თითო მოდელზე. კონკურენტულობა კოორდინაციას უწევს მორბენალი
  ორკესტრატორი, რათა თავიდან აიცილოს საერთო სახელმწიფო რბოლის პირობები. BLAS ბიბლიოთეკები მოქმედებს
  ერთძაფის რეჟიმი.
- **რიცხვები:** FP16 დაგროვება აკრძალულია. გამოიყენეთ FP32 შუალედური საშუალებები და დამჭერი
  აგრეგაციამდე გამოაქვს ოთხი ათობითი ადგილი.

## 3. კომიტეტის შემადგენლობა
საბაზისო კომიტეტი შეიცავს სამ მოდელის ოჯახს. მმართველობამ შეიძლება დაამატოს
მოდელები, მაგრამ მინიმალური კვორუმი უნდა დარჩეს დაკმაყოფილებული.

| ოჯახი | საბაზისო მოდელი | დანიშნულება |
|--------|---------------|---------|
| ხედვა | OpenCLIP ViT-H/14 (უსაფრთხოების დაზუსტება) | აღმოაჩენს ვიზუალურ კონტრაბანდას, ძალადობას, CSAM ინდიკატორებს. |
| მულტიმოდალური | LLaVA-1.6 34B უსაფრთხოება | იჭერს ტექსტს + გამოსახულების ურთიერთქმედებებს, კონტექსტურ მინიშნებებს, შევიწროებას. |
| აღქმადი | pHash + aHash + NeuralHash-lite ანსამბლი | ცნობილი ცუდი მასალის სწრაფი თითქმის დუბლიკატის აღმოჩენა და გახსენება. |

თითოეული მოდელის ჩანაწერი მიუთითებს:
- `model_id` (UUID)
- `artifact_digest` (BLAKE3-256 of OCI გამოსახულება)
- `weights_digest` (BLAKE3-256 of ONNX ან შერწყმული დამცავი ბლოკი)
- `opset` (უნდა ტოლი იყოს `17`)
- `weight` (კომიტეტის წონა, ნაგულისხმევი `1.0`)
- `critical_labels` (ეტიკეტების ნაკრები, რომელიც დაუყოვნებლივ ააქტიურებს `Escalate`-ს)
- `max_eval_ms` (დამცავი მოაჯირი დეტერმინისტული მცველებისთვის)

## 4. Norito მანიფესტები და შედეგები

### 4.1 საკომიტეტო მანიფესტი
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

### 4.2 შეფასების შედეგი
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

მორბენალმა უნდა გამოსცეს დეტერმინისტული `AiModerationDigestV1` (BLAKE3 მეტი
სერიული შედეგი) გამჭვირვალობის ჟურნალებისთვის და შედეგების დამატება მოდერაციისთვის
წიგნი, როდესაც განაჩენი არ არის `pass`.

### 4.3 საპირისპირო კორპუსის მანიფესტი

Gateway-ის ოპერატორები ახლა იღებენ კომპანიონ მანიფესტს, რომელიც აღიქვამს
კალიბრაციის ოპერაციებიდან მიღებული „ოჯახების“ ჩაშენება:

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

სქემა ცხოვრობს `crates/iroha_data_model/src/sorafs/moderation.rs`-ში და არის
დადასტურებულია `AdversarialCorpusManifestV1::validate()`-ით. მანიფესტი იძლევა საშუალებას
კარიბჭის უარმყოფელი ჩამტვირთავი `perceptual_family` ჩანაწერების შესავსებად, რომლებიც ბლოკავს
მთლიანი თითქმის დუბლიკატი კლასტერები ცალკეული ბაიტების ნაცვლად. გასაშვები მოწყობილობა
(`docs/examples/ai_moderation_perceptual_registry_202602.json`) აჩვენებს
მოსალოდნელი განლაგება და მიეწოდება პირდაპირ ნიმუშის კარიბჭის უარყოფის სიაში.

## 5. შესრულების მილსადენი
1. დატვირთვა `AiModerationManifestV1` მართვის DAG-დან. უარი თუ
   `runner_hash` ან `runtime_version` არ შეესაბამება განლაგებულ ბინარს.
2. მოდელის არტეფაქტების მოძიება OCI დაიჯესტის მეშვეობით, გადაამოწმეთ დაიჯესტები ჩატვირთვამდე.
3. შეფასების პარტიების აგება შინაარსის ტიპის მიხედვით; შეკვეთა უნდა დალაგდეს
   `(content_digest, manifest_id)` დეტერმინისტული აგრეგაციის უზრუნველსაყოფად.
4. შეასრულეთ თითოეული მოდელი მიღებული თესლით. აღქმის ჰეშებისთვის, დააკავშირეთ
   ანსამბლი უმრავლესობით -> ქულა `[0,1]`-ში.
5. შეაგროვეთ ქულები `combined_score`-ში შეწონილი დაჭერილი თანაფარდობის გამოყენებით:
   ```
   combined = Σ_i weight_i * clamp(score_i / threshold_i, 0, 1) / Σ_i weight_i
   ```
6. აწარმოე `ModerationVerdictV1`:
   - `escalate` თუ არის `critical_labels` ცეცხლი ან `combined ≥ thresholds.escalate`.
   - `quarantine` თუ ზემოთ `thresholds.quarantine`, მაგრამ ქვემოთ `escalate`.
   - `pass` სხვაგვარად.
7. გააგრძელეთ `AiModerationResultV1` და დააყენეთ ქვემოთ პროცესები:
   - საკარანტინო მომსახურება (თუ განაჩენი გამწვავდება/კარანტინი ხდება)
   - გამჭვირვალობის ჟურნალის დამწერი (`ModerationLedgerV1`)
   - ტელემეტრიის ექსპორტიორი

## 6. კალიბრაცია და შეფასება
- **მონაცემთა ნაკრები:** საბაზისო კალიბრაცია იყენებს შერეულ კორპუსს, რომელიც კურირებს პოლიტიკას
  გუნდის დამტკიცება. მითითება ჩაწერილია `calibration_dataset`-ში.
- **მეტრიკა:** გამოთვალეთ ბრიერის ქულა, მოსალოდნელი კალიბრაციის შეცდომა (ECE) და AUROC
  თითო მოდელი და კომბინირებული ვერდიქტი. ყოველთვიური რეკალიბრაცია უნდა შენარჩუნდეს
  `Brier ≤ 0.18` და `ECE ≤ 0.05`. SoraFS ანგარიშების ხეში შენახული შედეგები
  (მაგ., [2026 წლის თებერვლის კალიბრაცია] (../sorafs/reports/ai-moderation-calibration-202602.md)).
- **განრიგი:** ყოველთვიური რეკალიბრაცია (პირველი ორშაბათი). გადაუდებელი გადაკალიბრება
  ნებადართულია, თუ დრიფტი აფრთხილებს ცეცხლს.
- **პროცესი:** დეტერმინისტული შეფასების მილსადენის გაშვება კალიბრაციის კომპლექტზე,
  განაახლეთ `thresholds`, განაახლეთ მანიფესტი, დადგით ცვლილებები მმართველობის ხმის მიცემისთვის.

## 7. შეფუთვა და განლაგება
- შექმენით OCI სურათები `docker buildx bake -f docker/ai_moderation.hcl`-ის საშუალებით.
- სურათები მოიცავს:
  - ჩაკეტილი Python env (`poetry.lock`) ან Rust ორობითი `Cargo.lock`.
  - `models/` დირექტორია ჰეშირებული ONNX წონებით.
  - შესვლის წერტილი `run_moderation.py` (ან Rust ექვივალენტი), რომელიც ავლენს HTTP/gRPC API-ს.
- გამოაქვეყნეთ არტეფაქტები `registry.sora.net/ministry/ai-moderation/<model>@sha256:<digest>`-ზე.
- ორობითი გემები, როგორც `sorafs_ai_runner` კრატის ნაწილი. მილსადენის მშენებლობა
  ჩაშენებულია მანიფესტ ჰეშის ბინარში (გამოვლენილი `/v2/info`-ის მეშვეობით).

## 8. ტელემეტრია და დაკვირვება
- Prometheus მეტრიკა:
  - `moderation_requests_total{verdict}`
  - `moderation_model_score_bucket{model_id,label}`
  - `moderation_combined_score_bucket`
  - `moderation_inference_latency_seconds_bucket`
  - `moderation_runner_manifest_info{manifest_id, runtime_version}`
- ჟურნალები: JSON ხაზები `request_id`, `manifest_id`, `verdict` და დაიჯესტით
  შენახული შედეგიდან. ნედლეული ქულები რედაქტირდება ჟურნალებში ორ ათწილადამდე.
- `dashboards/grafana/ministry_moderation_overview.json`-ში შენახული დაფები
  (გამოქვეყნებულია კალიბრაციის პირველ მოხსენებასთან ერთად).
- გაფრთხილების ზღურბლები:
  - არ არის გადაყლაპვა (`moderation_requests_total` შეჩერებულია 10 წუთის განმავლობაში).
  - დრიფტის გამოვლენა (მოდელის საშუალო ქულა დელტა >20% მოძრავი 7 დღის საშუალოზე).
  - ცრუ დადებითი ჩამორჩენა (კარანტინის რიგი > 50 ელემენტი > 30 წუთის განმავლობაში).

## 9. მმართველობა და ცვლილებების კონტროლი
- მანიფესტებისთვის საჭიროა ორმაგი ხელმოწერა: სამინისტროს საბჭოს წევრი + მოდერაცია SRE
  ტყვია. `AiModerationManifestV1.governance_signature`-ში ჩაწერილი ხელმოწერები.
- ცვლილებები მოჰყვება `ModerationManifestChangeProposalV1`-დან Torii-მდე. ჰეშები
  შევიდა მმართველობის DAG-ში; განლაგება დაბლოკილია წინადადებამდე
  ამოქმედდა.
- Runner binaries ჩაშენება `runner_hash`; CI უარს ამბობს განლაგებაზე, თუ ჰეშები განსხვავდება.
- გამჭვირვალობა: ყოველკვირეული `ModerationScorecardV1` შემაჯამებელი მოცულობა, განაჩენის ნაზავი,
  და გასაჩივრების შედეგები. გამოქვეყნდა სორა პარლამენტის პორტალზე.

## 10. უსაფრთხოება და კონფიდენციალურობა
- შინაარსის დაიჯესტს იყენებს BLAKE3. ნედლი ტვირთი არასოდეს რჩება კარანტინის გარეთ.
- კარანტინზე წვდომა მოითხოვს მხოლოდ დროულად დამტკიცებას; ყველა წვდომა შესულია.
- Runner sandboxes არასანდო შინაარსს, ახორციელებს 512 MiB მეხსიერების ლიმიტს და 120 წმ.
  კედლის საათის მცველები.
- დიფერენციალური კონფიდენციალურობა აქ არ გამოიყენება; კარიბჭეები ეყრდნობა კარანტინს + აუდიტს
  სამუშაო პროცესების ნაცვლად. რედაქციის პოლიტიკა მიჰყვება კარიბჭის შესაბამისობის გეგმას
  (`docs/source/sorafs_gateway_compliance_plan.md`; პორტალი მოლოდინშია).

## 11. კალიბრაციის პუბლიკაცია (2026-02)
- **მანიფესტი:** `docs/examples/ai_moderation_calibration_manifest_202602.json`
  ჩაწერს მმართველობით ხელმოწერილი `AiModerationManifestV1` (ID
  `c9bdf0b2-63a3-4a90-8d70-908d119c2c7e`), მონაცემთა ბაზის მითითება
  `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`, მორბენალი ჰეში
  `ea3c0fd0ff4bd4510e94c7c293b261f601cc0c4f9fbacd99b0401d233a7cdc20` და
  2026-02 კალიბრაციის ზღურბლები (`quarantine = 0.42`, `escalate = 0.78`).
- **შეფასების დაფა:** `docs/examples/ai_moderation_calibration_scorecard_202602.json`
  პლუს ადამიანის მიერ წასაკითხი ანგარიში
  `[SoraFS Reports › AI Moderation Calibration 2026-02](../sorafs/reports/ai-moderation-calibration-202602.md)`
  დააფიქსირეთ Brier, ECE, AUROC და განაჩენის მიქსი ყველა მოდელისთვის. კომბინირებული მეტრიკა
  მიაღწია მიზნებს (`Brier = 0.126`, `ECE = 0.034`).
- ** დაფები და გაფრთხილებები:** `dashboards/grafana/ministry_moderation_overview.json`
  და `dashboards/alerts/ministry_moderation_rules.yml` (რეგრესიის ტესტებით
  `dashboards/alerts/tests/ministry_moderation_rules.test.yml`) უზრუნველყოფს
  ზომიერად მიღება/დაყოვნება/დრიფტის მონიტორინგის ამბავი საჭიროა გასაშვებად.## 12. განმეორებადობის სქემა და ვალიდატორი (MINFO-1b)
- კანონიკური Norito ტიპები ახლა ცხოვრობენ დანარჩენ SoraFS სქემასთან ერთად
  `crates/iroha_data_model/src/sorafs/moderation.rs`. The
  `ModerationReproManifestV1`/`ModerationReproBodyV1` სტრუქტურები აღბეჭდავს
  მანიფესტის UUID, მორბენალი ჰეში, მოდელის დაჯესტები, ბარიერის ნაკრები და სათესლე მასალა.
  `ModerationReproManifestV1::validate` ახორციელებს სქემის ვერსიას
  (`MODERATION_REPRO_MANIFEST_VERSION_V1`), უზრუნველყოფს ყველა მანიფესტს
  მინიმუმ ერთი მოდელი და ხელმომწერი და ამოწმებს თითოეულ `SignatureOf<ModerationReproBodyV1>`
  მანქანით წაკითხვადი რეზიუმეს დაბრუნებამდე.
- ოპერატორებს შეუძლიათ გამოიძახონ საერთო ვალიდატორის მეშვეობით
  `sorafs_cli moderation validate-repro --manifest=PATH [--format=json|norito]`
  (დანერგილია `crates/sorafs_orchestrator/src/bin/sorafs_cli.rs`-ში). CLI
  იღებს JSON არტეფაქტებს, რომლებიც გამოქვეყნებულია ქვეშ
  `docs/examples/ai_moderation_calibration_manifest_202602.json` ან ნედლეული
  Norito კოდირება და ბეჭდავს მოდელის/ხელმოწერის რაოდენობას მანიფესტთან ერთად
  დროის ანაბეჭდი დადასტურების წარმატებით დასრულების შემდეგ.
- კარიბჭეები და ავტომატიზაცია ერთსა და იმავე დამხმარეს უერთდებიან, რათა გამოვლინდეს გამეორებადობა
  შეიძლება უარყოფილი იყოს დეტერმინისტულად, როდესაც სქემები ტრიალებს, დაიჯესტები აკლია, ან
  ხელმოწერები ვერ გადამოწმებულია.
- საპირისპირო კორპუსის პაკეტები მიჰყვება იმავე ნიმუშს:
  `sorafs_cli moderation validate-corpus --manifest=PATH [--format=json|norito]`
  აანალიზებს `AdversarialCorpusManifestV1`-ს, ახორციელებს სქემის ვერსიას და უარს ამბობს
  მანიფესტებს, რომლებიც გამოტოვებენ ოჯახებს, ვარიანტებს ან თითის ანაბეჭდის მეტამონაცემებს. წარმატებული
  გაშვებები ასხივებენ გაცემული დროის ნიშანს, კოჰორტის ეტიკეტს და ოჯახის/ვარიანტის თვლებს
  ასე რომ, ოპერატორებს შეუძლიათ დაამაგრონ მტკიცებულება კარიბჭის უარმყოფელი ჩანაწერების განახლებამდე
  აღწერილია 4.3 ნაწილში.

## 13. გახსენით შემდგომი საქმიანობა
- ყოველთვიური რეკალიბრაციის ფანჯრები 2026-03-02 წლების შემდეგ კვლავაც მიჰყვება
  პროცედურა მე-6 ნაწილში; გამოაქვეყნეთ `ai-moderation-calibration-<YYYYMM>.md`
  განახლებული მანიფესტის/ქულების ბარათის პაკეტებთან ერთად SoraFS მოხსენებების ხის ქვეშ.
- MINFO-1b და MINFO-1c (განმეორებადობის მანიფესტის ვალიდატორები პლუს საპირისპირო
  კორპუსის რეესტრი) ცალკე რჩება საგზაო რუქაში მიკვლევა.