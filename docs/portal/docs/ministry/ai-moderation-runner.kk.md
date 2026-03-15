---
lang: kk
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

# AI Moderation Runner сипаттамасы

Бұл спецификация **MINFO-1 — AI құру құжаттамасының бөлігін орындайды
модерацияның базалық деңгейі**. Ол үшін детерминирленген орындау шартын анықтайды
Ақпарат министрлігінің модерация қызметі, сондықтан әрбір шлюз бірдей жұмыс істей алады
өтініштер мен ашықтық ағындары (SFM-4/SFM-4b) алдындағы құбырлар. Барлық мінез-құлық
мұнда сипатталған, егер анық ақпараттық деп белгіленбесе, нормативтік болып табылады.

## 1. Мақсаттар мен ауқым
- Шлюз мазмұнын бағалайтын қайталанатын модерация комитетін қамтамасыз етіңіз
  (нысандар, манифесттер, метадеректер, аудио) гетерогенді үлгілерді пайдалана отырып.
- Операторлар бойынша детерминирленген орындауға кепілдік: тіркелген опсет, септік
  токенизация, шектелген дәлдік және нұсқаланған артефактілер.
- Аудитке дайын артефактілерді жасаңыз: манифесттер, көрсеткіштер карталары, калибрлеу дәлелдері,
  және DAG басқару жүйесінде жариялауға жарамды ашықтық дайджесттері.
- SRE дрейфті, жалған позитивтерді және тоқтау уақытын анықтай алатындай беттік телеметрия
  пайдаланушының бастапқы деректерін жинамай.

## 2. Детерминистік орындау шарты
- **Орындалу уақыты:** AVX2 өшірілген және құрастырылған ONNX Runtime 1.19.x (CPU сервері)
  `--enable-extended-minimal-build` операция кодтар жинағын тұрақты ұстау үшін. CUDA/Металл
  орындау уақыттары өндірісте анық түрде рұқсат етілмейді.
- **Опсет:** `opset=17`. Жаңа опсеттерге бағытталған модельдер төмен түрлендірілуі керек
  және қабылдау алдында расталады.
- **Тұқым туындысы:** Әрбір бағалау RNG тұқымын алады
  `BLAKE3(content_digest || manifest_id || run_nonce)` мұнда `run_nonce` келеді
  басқару бекіткен манифесттен. Тұқым барлық стохастикалық компоненттерді қоректендіреді
  (сәулелік іздеу, түсіру ауыстырып-қосқыштары) сондықтан нәтижелер битке қайталанатын болады.
- **Threading:** Әр үлгіге бір жұмысшы. Сәйкестікті жүгіруші үйлестіреді
  ортақ күйдегі жарыс шарттарын болдырмау үшін оркестр. BLAS кітапханалары жұмыс істейді
  бір ағынды режим.
- **Сандар:** FP16 жинақтауға тыйым салынады. FP32 аралық өнімдерін қолданыңыз және қысыңыз
  жинақтау алдында төрт ондық таңбаға дейін шығарады.

№# 3. Комитеттің құрамы
Базалық комитет үш үлгілі отбасын қамтиды. Басқару қосуы мүмкін
үлгілер, бірақ ең аз кворум қанағаттандырылуы керек.

| Отбасы | Базалық үлгі | Мақсаты |
|--------|----------------|---------|
| Көрініс | OpenCLIP ViT-H/14 (қауіпсіздік дәл бапталған) | Көрнекі контрабанданы, зорлық-зомбылықты, CSAM көрсеткіштерін анықтайды. |
| мультимодальды | LLaVA-1.6 34B Қауіпсіздік | Мәтін + кескіннің өзара әрекеттесуін, контекстік белгілерді, қудалауды түсіреді. |
| Перцептивті | pHash + aHash + NeuralHash-lite ансамблі | Белгілі нашар материалды қайталанатын жылдам анықтау және қайта шақыру. |

Әрбір үлгі жазбасы мыналарды көрсетеді:
- `model_id` (UUID)
- `artifact_digest` (OCI кескінінің BLAKE3-256)
- `weights_digest` (ONNX BLAKE3-256 немесе біріктірілген сейфтензорлар блогы)
- `opset` (`17` тең болуы керек)
- `weight` (комитет салмағы, әдепкі `1.0`)
- `critical_labels` (`Escalate` дереу іске қосатын белгілер жиынтығы)
- `max_eval_ms` (детерминистік бақылаушыларға арналған қоршау)

## 4. Norito Манифесттер мен нәтижелер

### 4.1 Комитет манифесті
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

### 4.2 Бағалау нәтижесі
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

Жүгіруші `AiModerationDigestV1` (BLAKE3) детерминирленген сигнал шығаруы керек.
мөлдірлік журналдары үшін серияланған нәтиже) және модерацияға нәтижелерді қосыңыз
үкім `pass` болмаса, кітап.

### 4.3 Қарсыластық корпусының манифесті

Шлюз операторлары енді қабылдауды санайтын серіктес манифестті қабылдайды
калибрлеуден алынған "отбасыларды" хэш/енгізу:

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

Схема `crates/iroha_data_model/src/sorafs/moderation.rs` ішінде тұрады және солай
`AdversarialCorpusManifestV1::validate()` арқылы расталған. Манифест мүмкіндік береді
блоктайтын `perceptual_family` жазбаларын толтыру үшін шлюзді жоққа шығару тізімін жүктеуші
жеке байттардың орнына толық қайталанатын кластерлер. Жүгіруге болатын қондырғы
(`docs/examples/ai_moderation_perceptual_registry_202602.json`) көрсетеді
күтілетін орналасу және арналар тікелей үлгі шлюзінің бас тарту тізіміне.

## 5. Орындау құбыры
1. DAG басқару жүйесінен `AiModerationManifestV1` жүктеңіз. Егер бас тарту
   `runner_hash` немесе `runtime_version` орналастырылған екілік файлға сәйкес келмейді.
2. Жүктеу алдында дайджесттерді тексеріп, OCI дайджесті арқылы үлгі артефактілерін алыңыз.
3. Мазмұн түрі бойынша бағалау топтамаларын құру; тапсырыс бойынша сұрыптау керек
   Детерминирленген біріктіруді қамтамасыз ету үшін `(content_digest, manifest_id)`.
4. Әрбір үлгіні алынған тұқыммен орындаңыз. Перцептивті хэштер үшін біріктіріңіз
   ансамбль көпшілік дауыспен -> ұпай `[0,1]`.
5. Салмақталған қысқартылған қатынасты пайдаланып `combined_score` ұпайларын біріктіріңіз:
   ```
   combined = Σ_i weight_i * clamp(score_i / threshold_i, 0, 1) / Σ_i weight_i
   ```
6. `ModerationVerdictV1` шығарыңыз:
   - `escalate`, егер бар болса `critical_labels` немесе `combined ≥ thresholds.escalate`.
   - `quarantine`, егер `thresholds.quarantine` жоғары болса, бірақ `escalate` төмен болса.
   - `pass` басқаша.
7. `AiModerationResultV1` және төменгі ағындық процестерді кезекке қою:
   - Карантиндік қызмет (егер үкім күшейсе/карантин болса)
   - Мөлдірлік журналының жазушысы (`ModerationLedgerV1`)
   - Телеметрия экспорттаушысы

## 6. Калибрлеу және бағалау
- **Деректер жинағы:** Негізгі калибрлеу саясатпен таңдалған аралас корпусты пайдаланады
  команданы мақұлдау. Анықтама `calibration_dataset` ішінде жазылған.
- **Көрсеткіштер:** Есептеу Бриер ұпайы, күтілетін калибрлеу қатесі (ECE) және AUROC
  үлгі және біріктірілген үкім бойынша. Ай сайынғы қайта калибрлеуді САҚТАУ КЕРЕК
  `Brier ≤ 0.18` және `ECE ≤ 0.05`. SoraFS есептер тармағында сақталған нәтижелер
  (мысалы, [2026 жылғы ақпандағы калибрлеу](../sorafs/reports/ai-moderation-calibration-202602.md)).
- **Кесте:** Ай сайынғы қайта калибрлеу (бірінші дүйсенбі). Төтенше жағдайда қайта калибрлеу
  дрейф өрт туралы хабарлаған жағдайда рұқсат етіледі.
- **Процесс:** Калибрлеу жинағында детерминирленген бағалау құбырын іске қосу,
  `thresholds` қалпына келтіру, манифестті жаңарту, басқару дауысы үшін кезеңдік өзгерістер.

## 7. Орау және орналастыру
- `docker buildx bake -f docker/ai_moderation.hcl` арқылы OCI кескіндерін құрастырыңыз.
- Суреттерге мыналар кіреді:
  - Құлыпталған Python env (`poetry.lock`) немесе Rust екілік `Cargo.lock`.
  - ONNX салмақтары хэштелген `models/` каталогы.
  - HTTP/gRPC API ашатын `run_moderation.py` (немесе Rust баламасы) кіру нүктесі.
- `registry.sora.net/ministry/ai-moderation/<model>@sha256:<digest>` артефактілерін жариялау.
- `sorafs_ai_runner` жәшігінің бөлігі ретінде жүгіруші екілік кемелер. Құрылыс құбыры
  манифест хэшін екілік жүйеге енгізеді (`/v1/info` арқылы көрсетіледі).

№# 8. Телеметрия және бақылау мүмкіндігі
- Prometheus көрсеткіштері:
  - `moderation_requests_total{verdict}`
  - `moderation_model_score_bucket{model_id,label}`
  - `moderation_combined_score_bucket`
  - `moderation_inference_latency_seconds_bucket`
  - `moderation_runner_manifest_info{manifest_id, runtime_version}`
- Журналдар: `request_id`, `manifest_id`, `verdict` және дайджест бар JSON жолдары
  сақталған нәтиже. Шикі ұпайлар журналдардағы екі ондық таңбаға дейін түзетіледі.
- `dashboards/grafana/ministry_moderation_overview.json` ішінде сақталған бақылау тақталары
  (бірінші калибрлеу есебімен бірге жарияланған).
- Ескерту шектері:
  - Жетіспейтін қабылдау (`moderation_requests_total` 10 минутқа тоқтап қалды).
  - Дрейфті анықтау (үлгінің орташа көрсеткіші дельтаның 7 күндік орташа жылжумен салыстырғанда >20%).
  - Жалған-оң артта қалу (карантиндік кезек > 30 минутқа 50 элемент).

## 9. Басқару және өзгерістерді бақылау
- Манифесттерге қосарлы қол қою қажет: Министрлік кеңесінің мүшесі + модераторлық SRE
  қорғасын. `AiModerationManifestV1.governance_signature` ішінде жазылған қолдар.
- Өзгерістер `ModerationManifestChangeProposalV1` арқылы Torii. Хэштер
  DAG басқару жүйесіне кірді; орналастыру ұсыныс жасалғанша блокталады
  қабылданған.
- `runner_hash` ендірілген жүгіргіш екілік файлдары; CI хэштер әртүрлі болса, орналастырудан бас тартады.
- Транспаренттілік: апта сайынғы `ModerationScorecardV1` жиынтық көлемі, үкімдер жиынтығы,
  және апелляция нәтижелері. Сора Парламент порталында жарияланған.

## 10. Қауіпсіздік және құпиялылық
- Мазмұн дайджесттері BLAKE3 пайдаланады. Шикізат жүктемелері ешқашан карантиннен тыс жерде сақталмайды.
- Карантинге қол жеткізу үшін дәл уақытында мақұлдау қажет; барлық кірулер тіркелді.
- Runner 512 МБ жад шектеулері мен 120 секундты қамтамасыз ете отырып, сенімсіз мазмұнды құм жәшіктерге айналдырады.
  қабырға сағатының күзетшілері.
- Мұнда дифференциалды құпиялылық ҚОЛДАНЫЛМАЙДЫ; шлюздер карантин + аудитке сүйенеді
  орнына жұмыс процестері. Өңдеу саясаттары шлюз сәйкестік жоспарына сәйкес келеді
  (`docs/source/sorafs_gateway_compliance_plan.md`; портал көшірмесі күтілуде).

№# 11. Калибрлеу басылымы (2026-02)
- **Манифест:** `docs/examples/ai_moderation_calibration_manifest_202602.json`
  басқару қолы қойылған `AiModerationManifestV1` (ID
  `c9bdf0b2-63a3-4a90-8d70-908d119c2c7e`), деректер жиынының анықтамасы
  `c0956583-355a-43cc-9a60-e3a5d9a0f7d0`, жүгіруші хэші
  `ea3c0fd0ff4bd4510e94c7c293b261f601cc0c4f9fbacd99b0401d233a7cdc20`, және
  2026-02 калибрлеу шектері (`quarantine = 0.42`, `escalate = 0.78`).
- **Көрсеткіштер тақтасы:** `docs/examples/ai_moderation_calibration_scorecard_202602.json`
  плюс адам оқи алатын есеп
  `[SoraFS Reports › AI Moderation Calibration 2026-02](../sorafs/reports/ai-moderation-calibration-202602.md)`
  Brier, ECE, AUROC және әрбір модель үшін шешім қоспасын түсіріңіз. Біріктірілген көрсеткіштер
  мақсаттарға жетті (`Brier = 0.126`, `ECE = 0.034`).
- **Бақылау тақталары және ескертулер:** `dashboards/grafana/ministry_moderation_overview.json`
  және `dashboards/alerts/ministry_moderation_rules.yml` (регрессия сынақтарымен
  `dashboards/alerts/tests/ministry_moderation_rules.test.yml`) қамтамасыз етеді
  шығару үшін қажет модерацияны қабылдау/кідіріс/дрейф бақылау тарихы.## 12. Қайта шығару схемасы және валидатор (MINFO-1b)
- Канондық Norito түрлері енді SoraFS схемасының қалған бөлігімен қатар тұрады.
  `crates/iroha_data_model/src/sorafs/moderation.rs`. The
  `ModerationReproManifestV1`/`ModerationReproBodyV1` құрылымдары
  манифест UUID, жүгіруші хэш, үлгі дайджесттері, шекті жиынтық және тұқым материалы.
  `ModerationReproManifestV1::validate` схема нұсқасын орындайды
  (`MODERATION_REPRO_MANIFEST_VERSION_V1`), әрбір манифесттің орындалуын қамтамасыз етеді
  кемінде бір үлгі және қол қоюшы және әрбір `SignatureOf<ModerationReproBodyV1>` тексереді
  машина оқылатын қорытындыны қайтармас бұрын.
- Операторлар ортақ валидаторды арқылы шақыра алады
  `sorafs_cli moderation validate-repro --manifest=PATH [--format=json|norito]`
  (`crates/sorafs_orchestrator/src/bin/sorafs_cli.rs` енгізілген). CLI
  астында жарияланған JSON артефактілерін қабылдайды
  `docs/examples/ai_moderation_calibration_manifest_202602.json` немесе шикі
  Norito кодтау және манифестпен қатар модель/қолтаңба сандарын басып шығарады
  тексеру сәтті болған кезде уақыт белгісі.
- Шлюздер мен автоматика бір көмекшіге қосылады, осылайша қайталану мүмкіндігі көрінеді
  схемалар ауытқығанда, дайджесттер жоқ болғанда немесе анықталмалы түрде қабылданбауы мүмкін
  қолдар тексерілмейді.
- Қарсылас корпус байламдары бірдей үлгіге сәйкес келеді:
  `sorafs_cli moderation validate-corpus --manifest=PATH [--format=json|norito]`
  `AdversarialCorpusManifestV1` талдайды, схема нұсқасын орындайды және бас тартады
  отбасыларды, нұсқаларды немесе саусақ ізі метадеректерін өткізбейтін манифесттер. Сәтті
  жүгірулер уақыт белгісін, когорт белгісін және отбасы/нұсқа сандарын шығарады
  сондықтан операторлар шлюз бас тарту тізімі жазбаларын жаңарту алдында дәлелдерді бекіте алады
  4.3 бөлімінде сипатталған.

## 13. Бақылауларды ашыңыз
- 2026-03-02 бастап ай сайынғы қайта калибрлеу терезелері орындалады
  6-бөлімдегі тәртіп; `ai-moderation-calibration-<YYYYMM>.md` жариялау
  SoraFS есептер тармағының астындағы жаңартылған манифест/көрсеткіштер жүйесі топтамаларымен қатар.
- MINFO-1b және MINFO-1c (қайта шығару манифестінің валидаторлары және қарсыластар
  корпус тізілімі) жол картасында бөлек бақыланады.