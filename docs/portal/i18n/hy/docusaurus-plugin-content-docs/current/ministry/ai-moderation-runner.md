---
lang: hy
direction: ltr
source: docs/portal/docs/ministry/ai-moderation-runner.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: AI Moderation Runner Specification
summary: Deterministic moderation committee design for the Ministry of Information (MINFO-1) deliverable.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# AI Moderation Runner-ի ճշգրտում

Այս հատկանիշը բավարարում է **MINFO-1-ի փաստաթղթային մասը — Ստեղծել AI
չափավոր ելակետ**. Այն սահմանում է դետերմինիստական կատարողական պայմանագիրը
Տեղեկատվության նախարարության մոդերատորական ծառայություն, որպեսզի յուրաքանչյուր դարպաս կարողանա գործել նույնական
խողովակաշարերը մինչև բողոքարկումները և թափանցիկության հոսքերը (SFM-4/SFM-4b): Ամբողջ վարքագիծը
այստեղ նկարագրվածը նորմատիվ է, եթե բացահայտորեն նշված չէ որպես տեղեկատվական:

## 1. Նպատակներ և շրջանակներ
- Տրամադրել վերարտադրվող մոդերատորական հանձնաժողով, որը գնահատում է դարպասների բովանդակությունը
  (օբյեկտներ, մանիֆեստներ, մետատվյալներ, աուդիո)՝ օգտագործելով տարասեռ մոդելներ:
- Երաշխավորել դետերմինիստական կատարումը օպերատորների միջև
  նշանավորում, սահմանափակ ճշգրտություն և տարբերակված արտեֆակտներ:
- Արտադրել աուդիտի համար պատրաստ արտեֆակտներ՝ մանիֆեստներ, գնահատականների քարտեր, չափաբերման ապացույցներ,
  և թափանցիկության դիջիջեսները, որոնք հարմար են կառավարման DAG-ում հրապարակման համար:
- Մակերեւութային հեռաչափություն, որպեսզի SRE-ները կարողանան հայտնաբերել դրեյֆը, կեղծ պոզիտիվները և խափանումները
  առանց օգտագործողի հում տվյալների հավաքման:

## 2. Դետերմինիստական կատարման պայմանագիր
- **Գործարկման ժամանակ.** ONNX Runtime 1.19.x (CPU backend) կազմված է AVX2 անջատված և
  `--enable-extended-minimal-build`՝ opcode հավաքածուն ֆիքսված պահելու համար: ԿՈՒԴԱ/Մետաղ
  գործարկման ժամանակները բացահայտորեն արգելված են արտադրության մեջ:
- **Օպսեթ:** `opset=17`. Նոր օպսեթներին ուղղված մոդելները պետք է փոխակերպվեն ներքև
  և վավերացվել մինչև ընդունելությունը:
- **Սերմերի ստացում.** Յուրաքանչյուր գնահատում բխում է RNG սերմից
  `BLAKE3(content_digest || manifest_id || run_nonce)` որտեղ գալիս է `run_nonce`
  կառավարման կողմից հաստատված մանիֆեստից։ Սերմերը կերակրում են բոլոր ստոխաստիկ բաղադրիչները
  (ճառագայթային որոնում, անջատման անջատումներ), այնպես որ արդյունքները վերարտադրելի են բիթ առ բիթ:
- **Treading. ** Մեկ աշխատող յուրաքանչյուր մոդելի համար: Միաժամանակությունը համակարգվում է վազորդի կողմից
  խմբավար՝ խուսափելու ընդհանուր պետական մրցավազքի պայմաններից: BLAS գրադարանները գործում են
  մեկ թելային ռեժիմ:
- **Թվեր.** FP16 կուտակումն արգելված է: Օգտագործեք FP32 միջանկյալ նյութեր և սեղմակ
  Արդյունքները չորս տասնորդական թվերով մինչև ագրեգացումը:

## 3. Հանձնաժողովի կազմը
Ելակետային հանձնաժողովը պարունակում է երեք մոդելային ընտանիքներ: Կառավարումը կարող է ավելացնել
մոդելները, սակայն նվազագույն քվորումը պետք է մնա բավարարված:

| Ընտանիք | Ելակետային մոդել | Նպատակը |
|--------|---------------|---------|
| Տեսիլք | OpenCLIP ViT-H/14 (անվտանգության ճշգրտված) | Հայտնաբերում է տեսողական մաքսանենգություն, բռնություն, CSAM ցուցանիշներ: |
| Մուլտիմոդալ | LLaVA-1.6 34B Անվտանգություն | Լուսանկարում է տեքստ + պատկեր փոխազդեցությունները, համատեքստային նշանները, ոտնձգությունները: |
| Ընկալողական | pHash + aHash + NeuralHash-lite համույթ | Հայտնի վատ նյութերի արագ գրեթե կրկնօրինակ հայտնաբերում և հետկանչում: |

Յուրաքանչյուր մոդելի մուտքագրում նշվում է.
- `model_id` (UUID)
- `artifact_digest` (BLAKE3-256 OCI պատկերի)
- `weights_digest` (BLAKE3-256 ONNX-ից կամ միաձուլված պաշտպանիչ սարքերի բլիթ)
- `opset` (պետք է հավասար լինի `17`)
- `weight` (հանձնաժողովի քաշը, լռելյայն `1.0`)
- `critical_labels` (պիտակների մի շարք, որոնք անմիջապես գործարկում են `Escalate`)
- `max_eval_ms` (վահանակավոր հսկիչ շների համար)

## 4. Norito Դրսևորումներ և արդյունքներ

### 4.1 Կոմիտեի մանիֆեստ
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

### 4.2 Գնահատման արդյունք
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

Վազողը ՊԵՏՔ Է արձակի դետերմինիստական `AiModerationDigestV1` (BLAKE3)
սերիական արդյունք) թափանցիկության տեղեկամատյանների համար և արդյունքները կցեք մոդերատորին
մատյան, երբ դատավճիռը `pass` չէ:

### 4.3 Հակառակորդ կորպուսի մանիֆեստ

Gateway-ի օպերատորներն այժմ ընդունում են ուղեկից մանիֆեստ, որը թվարկում է ընկալումը
«ընտանիքների» ներկառուցում, որոնք ստացվել են ստուգաչափման փորձարկումներից.

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

Սխեման ապրում է `crates/iroha_data_model/src/sorafs/moderation.rs`-ում և գտնվում է
վավերացված `AdversarialCorpusManifestV1::validate()`-ի միջոցով: Մանիֆեստը թույլ է տալիս
gateway denylist բեռնիչը՝ `perceptual_family` մուտքագրումները, որոնք արգելափակում են
ամբողջ գրեթե կրկնվող կլաստերներ՝ առանձին բայթերի փոխարեն: Գործող սարք
(`docs/examples/ai_moderation_perceptual_registry_202602.json`) ցույց է տալիս
ակնկալվող դասավորությունը և անմիջապես սնվում է ընտրանքային դարպասների հերքման ցանկում:

## 5. Կատարման խողովակաշար
1. Բեռնել `AiModerationManifestV1` կառավարման DAG-ից: Մերժել, եթե
   `runner_hash` կամ `runtime_version` անհամապատասխանում են տեղակայված երկուականին:
2. Վերցրեք մոդելային արտեֆակտները OCI digest-ի միջոցով՝ ստուգելով մարսումները նախքան բեռնումը:
3. Կառուցել գնահատման խմբաքանակներ ըստ բովանդակության տեսակի; պատվերը պետք է դասավորվի ըստ
   `(content_digest, manifest_id)`՝ դետերմինիստական ագրեգացիա ապահովելու համար:
4. Կատարեք յուրաքանչյուր մոդել ստացված սերմով: Ընկալվող հեշերի համար միավորել
   անսամբլը ձայների մեծամասնությամբ -> միավոր `[0,1]`:
5. Միավորել միավորները `combined_score`-ում՝ օգտագործելով կշռված կտրված հարաբերակցությունը.
   ```
   combined = Σ_i weight_i * clamp(score_i / threshold_i, 0, 1) / Σ_i weight_i
   ```
6. Արտադրել `ModerationVerdictV1`:
   - `escalate` եթե կա `critical_labels` կրակ կամ `combined ≥ thresholds.escalate`:
   - `quarantine`, եթե `thresholds.quarantine`-ից բարձր, բայց `escalate`-ից ցածր:
   - `pass` հակառակ դեպքում:
7. Պահպանեք `AiModerationResultV1` և հերթագրեք ներքևում գտնվող գործընթացները.
   - Կարանտինային ծառայություն (եթե դատավճիռը մեծանում է/կարանտինում)
   - Թափանցիկության մատյան գրող (`ModerationLedgerV1`)
   - Հեռուստաչափություն արտահանող

## 6. Չափորոշում և գնահատում
- **Տվյալների հավաքածուներ.** Ելակետային տրամաչափումն օգտագործում է քաղաքականության հետ համադրված խառը կորպուսը
  թիմի հաստատումը: Հղում գրանցված է `calibration_dataset`-ում:
- ** Չափումներ. ** Հաշվարկել Brier միավորը, ակնկալվող չափաբերման սխալը (ECE) և AUROC
  ըստ մոդելի և համակցված դատավճռի: Ամսական վերահաշվառումը ՊԵՏՔ Է պահպանվի
  `Brier ≤ 0.18` և `ECE ≤ 0.05`: SoraFS հաշվետվությունների ծառում պահված արդյունքները
  (օրինակ, [Փետրվար 2026 չափորոշում] (../sorafs/reports/ai-moderation-calibration-202602.md)):
- **Ժամանակացույց.** Ամսական վերահաշվառում (առաջին երկուշաբթի): Արտակարգ իրավիճակների վերահաշվառում
  թույլատրվում է, եթե դրեյֆը ահազանգում է կրակի մասին:
- **Գործընթաց.** Գործարկել դետերմինիստական գնահատման խողովակաշարը տրամաչափման հավաքածուի վրա,
  վերականգնել `thresholds`-ը, թարմացնել մանիֆեստը, փոփոխություններ կատարել կառավարման քվեարկության համար:

## 7. Փաթեթավորում և տեղակայում
- Կառուցեք OCI պատկերներ `docker buildx bake -f docker/ai_moderation.hcl`-ի միջոցով:
- Պատկերները ներառում են.
  - Կողպված Python env (`poetry.lock`) կամ Rust երկուական `Cargo.lock`:
  - `models/` գրացուցակ հաշված ONNX կշիռներով:
  - Մուտքի կետ `run_moderation.py` (կամ Rust համարժեք), որը բացահայտում է HTTP/gRPC API-ն:
- Հրապարակեք արտեֆակտները `registry.sora.net/ministry/ai-moderation/<model>@sha256:<digest>`-ում:
- Runner երկուական նավերը որպես `sorafs_ai_runner` տուփի մաս: Կառուցել խողովակաշարը
  ներկառուցում է մանիֆեստային հեշը երկուականում (բացահայտվում է `/v1/info`-ի միջոցով):

## 8. Հեռաչափություն և դիտելիություն
- Prometheus չափումներ.
  - `moderation_requests_total{verdict}`
  - `moderation_model_score_bucket{model_id,label}`
  - `moderation_combined_score_bucket`
  - `moderation_inference_latency_seconds_bucket`
  - `moderation_runner_manifest_info{manifest_id, runtime_version}`
- Տեղեկամատյաններ՝ JSON գծեր `request_id`, `manifest_id`, `verdict` և ներածություն
  պահպանված արդյունքից: Հում միավորները վերագրվում են տեղեկամատյաններում երկու տասնորդական թվերով:
- `dashboards/grafana/ministry_moderation_overview.json`-ում պահվող վահանակներ
  (հրապարակված է առաջին ստուգաչափման զեկույցին կից):
- Զգուշացման շեմեր.
  - Կուլ չկա (`moderation_requests_total` կանգ է առել 10 րոպեով):
  - Դրեյֆի հայտնաբերում (մոդելի միջին գնահատականը դելտա >20% ընդդեմ շարժվող 7-օրյա միջինի):
  - Կեղծ-դրական հետքառություն (կարանտինային հերթ > 50 ապրանք ավելի քան 30 րոպե):

## 9. Կառավարում և փոփոխության վերահսկում
- Մանիֆեստները պահանջում են երկակի ստորագրություն՝ նախարարության խորհրդի անդամ + մոդերատոր SRE
  կապար. `AiModerationManifestV1.governance_signature`-ում գրանցված ստորագրությունները:
- Փոփոխությունները հաջորդում են `ModerationManifestChangeProposalV1`-ից մինչև Torii: Հեշ
  մտել է կառավարման DAG; տեղակայումն արգելափակված է մինչև առաջարկը
  ընդունվել է.
- Runner binaries ներդրում `runner_hash`; CI-ն հրաժարվում է տեղակայումից, եթե հեշերը տարբերվում են:
- Թափանցիկություն. շաբաթական `ModerationScorecardV1` ամփոփող ծավալ, դատավճիռների խառնուրդ,
  և բողոքարկելու արդյունքները: Հրապարակվել է Sora Parliament պորտալում։

## 10. Անվտանգություն և գաղտնիություն
- Բովանդակության բովանդակությունը օգտագործում է BLAKE3: Հում բեռները երբեք չեն պահպանվում կարանտինից դուրս:
- Կարանտին մուտք գործելը պահանջում է ժամանակին հաստատումներ. բոլոր մուտքերը գրանցված են:
- Runner Sandboxes-ը անվստահելի բովանդակություն է պարունակում՝ 512 ՄԲ հիշողության սահմանափակումներ և 120 վրկ
  պատի ժամացույցի պահակներ.
- Դիֆերենցիալ գաղտնիությունը ՉԻ կիրառվում այստեղ. դարպասները ապավինում են կարանտինին + աուդիտին
  փոխարենը աշխատանքային հոսքեր: Վերամշակման քաղաքականությունը հետևում է դարպասների համապատասխանության ծրագրին
  (`docs/source/sorafs_gateway_compliance_plan.md`; պորտալի պատճենը սպասվում է):

## 11. Calibration Publication (2026-02)
- **Դրսևորում.** `docs/examples/ai_moderation_calibration_manifest_202602.json`
  գրանցում է կառավարման կողմից ստորագրված `AiModerationManifestV1` (ID
  `c9bdf0b2-63a3-4a90-8d70-908d119c2c7e`), տվյալների բազայի հղում
  `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`, վազող հեշ
  `ea3c0fd0ff4bd4510e94c7c293b261f601cc0c4f9fbacd99b0401d233a7cdc20` և
  2026-02 տրամաչափման շեմեր (`quarantine = 0.42`, `escalate = 0.78`):
- ** Ցուցատախտակ՝ ** `docs/examples/ai_moderation_calibration_scorecard_202602.json`
  գումարած մարդու կողմից ընթեռնելի զեկույցը
  `[SoraFS Reports › AI Moderation Calibration 2026-02](../sorafs/reports/ai-moderation-calibration-202602.md)`
  գրավել Brier, ECE, AUROC և դատավճիռների խառնուրդ յուրաքանչյուր մոդելի համար: Համակցված չափումներ
  հասել է թիրախներին (`Brier = 0.126`, `ECE = 0.034`):
- ** Վահանակներ և ծանուցումներ՝ ** `dashboards/grafana/ministry_moderation_overview.json`
  և `dashboards/alerts/ministry_moderation_rules.yml` (ռեգեսիոն թեստերով
  `dashboards/alerts/tests/ministry_moderation_rules.test.yml`) ապահովում է
  Օգտագործման համար պահանջվում է չափավոր ներթափանցում/ուշացում/դրեյֆ մոնիտորինգի պատմություն:## 12. Վերարտադրելիության սխեմա և վավերացնող (MINFO-1b)
- Կանոնական Norito տեսակներն այժմ ապրում են մնացած SoraFS սխեմայի կողքին
  `crates/iroha_data_model/src/sorafs/moderation.rs`. Այն
  `ModerationReproManifestV1`/`ModerationReproBodyV1` կառուցվածքները գրավում են
  մանիֆեստի UUID, վազող հեշ, մոդելային բովանդակություն, շեմերի հավաքածու և սերմանյութ:
  `ModerationReproManifestV1::validate`-ը պարտադրում է սխեմայի տարբերակը
  (`MODERATION_REPRO_MANIFEST_VERSION_V1`), ապահովում է յուրաքանչյուր մանիֆեստի իրականացում
  առնվազն մեկ մոդել և ստորագրող, և ստուգում է յուրաքանչյուր `SignatureOf<ModerationReproBodyV1>`
  նախքան մեքենայական ընթեռնելի ամփոփագիրը վերադարձնելը:
- Օպերատորները կարող են կանչել ընդհանուր վավերացուցիչը միջոցով
  `sorafs_cli moderation validate-repro --manifest=PATH [--format=json|norito]`
  (իրականացվել է `crates/sorafs_orchestrator/src/bin/sorafs_cli.rs`-ում): CLI
  ընդունում է JSON արտեֆակտները, որոնք հրապարակվել են տակ
  `docs/examples/ai_moderation_calibration_manifest_202602.json` կամ հում
  Norito կոդավորում և տպում է մոդելի/ստորագրության թվերը մանիֆեստի կողքին
  ժամանակի դրոշմ, երբ վավերացումը հաջողվի:
- Դարպասները և ավտոմատացումը միացնում են նույն օգնականին, որպեսզի վերարտադրելիությունը դրսևորվի
  կարող է դետերմինիստականորեն մերժվել, երբ սխեմաները տեղաշարժվում են, մարսումները բացակայում են, կամ
  ստորագրությունները չեն ստուգվում:
- Հակառակորդ կորպուսի փաթեթները հետևում են նույն օրինակին.
  `sorafs_cli moderation validate-corpus --manifest=PATH [--format=json|norito]`
  վերլուծում է `AdversarialCorpusManifestV1`-ը, պարտադրում է սխեմայի տարբերակը և մերժում
  դրսևորումներ, որոնք բաց թողնում են ընտանիքները, տարբերակները կամ մատնահետքերի մետատվյալները: Հաջողակ
  գործարկումները թողարկում են թողարկված ժամանակի դրոշմակնիքը, կոհորտային պիտակը և ընտանիքի/տարբերակների հաշվարկները
  այնպես որ օպերատորները կարող են ամրացնել ապացույցները նախքան դարպասների հերքման ցուցակի գրառումները թարմացնելը
  նկարագրված է 4.3 բաժնում:

## 13. Բացեք Հետագա գործողությունները
- 2026-03-02-ից հետո ամսական վերահաշվառման պատուհանները շարունակում են հետևել ս.թ
  ընթացակարգը Բաժին 6-ում; հրապարակել `ai-moderation-calibration-<YYYYMM>.md`
  SoraFS հաշվետվությունների ծառի տակ թարմացված մանիֆեստի/հաշվարկային փաթեթների կողքին:
- MINFO-1b և MINFO-1c (վերարտադրելիության մանիֆեստի վավերացնողներ և հակառակորդներ
  կորպուսի ռեգիստրը) մնում են առանձին հետևել ճանապարհային քարտեզում: