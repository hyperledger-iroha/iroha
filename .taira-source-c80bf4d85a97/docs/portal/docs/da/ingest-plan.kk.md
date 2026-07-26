---
lang: kk
direction: ltr
source: docs/portal/docs/da/ingest-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 710286691d09a5707829a36ca98ed24a6af5c5629e708dd7b1bd0f01db4e31c1
source_last_modified: "2026-01-22T14:35:36.737834+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

тақырып: Деректердің қолжетімділігін қабылдау жоспары
sidebar_label: қабылдау жоспары
сипаттама: схема, API беті және Torii блоб қабылдауға арналған тексеру жоспары.
---

:::ескерту Канондық дереккөз
:::

# Sora Nexus Деректердің қолжетімділігін қабылдау жоспары

_Жазылған: 2026-02-20 - Иесі: Негізгі хаттама WG / Сақтау тобы / DA WG_

DA-2 жұмыс ағыны Norito шығаратын blob қабылдау API көмегімен Torii кеңейтеді
метадеректер мен тұқымдар SoraFS репликациясы. Бұл құжат ұсынылғанды қамтиды
схемасы, API беті және валидация ағыны, сондықтан іске асыру онсыз жалғаса алады
көрнекті модельдеулерді блоктау (DA-1 кейінгі әрекеттер). Барлық пайдалы жүк пішімдері МІНДЕТТІ
Norito кодектерін пайдаланыңыз; серде/JSON қалпына келтіруге рұқсат етілмейді.

## Мақсаттар

- Үлкен бөртпелерді қабылдаңыз (Тайкай сегменттері, жолақтар, басқару артефактілері)
  Torii бойынша анықтаушы.
- Блобты, кодек параметрлерін сипаттайтын канондық Norito манифесттерін жасаңыз,
  өшіру профилі және сақтау саясаты.
- SoraFS ыстық сақтау және кезекті репликация тапсырмаларындағы үзінді метадеректерін сақтау.
- SoraFS тізіліміне және басқаруға PIN ниеттерін + саясат тегтерін жариялау
  бақылаушылар.
- Клиенттер жарияланымның детерминирленген дәлелін қайтарып алуы үшін рұқсат алу түбіртектерін көрсетіңіз.

## API беті (Torii)

```
POST /v1/da/ingest
Content-Type: application/norito+v1
```

Пайдалы жүк - Norito кодталған `DaIngestRequest`. Жауаптарды қолдану
`application/norito+v1` және `DaIngestReceipt` қайтарады.

| Жауап | Мағынасы |
| --- | --- |
| 202 Қабылданған | Бөлшектеу/қайталау үшін кезекке қойылған Blob; түбіртек қайтарылды. |
| 400 қате сұрау | Схеманы/өлшемді бұзу (валидация тексерулерін қараңыз). |
| 401 Рұқсат етілмеген | API белгісі жоқ/жарамсыз. |
| 409 Қақтығыс | Метадеректер сәйкес келмейтін `client_blob_id` көшірмесі. |
| 413 Пайдалы жүктеме тым үлкен | Конфигурацияланған блок ұзындығы шегінен асып кетеді. |
| 429 Тым көп сұраулар | Тарифтік шектеуге жетті. |
| 500 Ішкі қате | Күтпеген сәтсіздік (жүйеге жазылған + ескерту). |

## Ұсынылған Norito схемасы

```rust
/// Top-level ingest request.
pub struct DaIngestRequest {
    pub client_blob_id: BlobDigest,      // submitter-chosen identifier
    pub lane_id: LaneId,                 // target Nexus lane
    pub epoch: u64,                      // epoch blob belongs to
    pub sequence: u64,                   // monotonic sequence per (lane, epoch)
    pub blob_class: BlobClass,           // TaikaiSegment, GovernanceArtifact, etc.
    pub codec: BlobCodec,                // e.g. "cmaf", "pdf", "norito-batch"
    pub erasure_profile: ErasureProfile, // parity configuration
    pub retention_policy: RetentionPolicy,
    pub chunk_size: u32,                 // bytes (must align with profile)
    pub total_size: u64,
    pub compression: Compression,        // Identity, gzip, deflate, or zstd
    pub norito_manifest: Option<Vec<u8>>, // optional pre-built manifest
    pub payload: Vec<u8>,                 // raw blob data (<= configured limit)
    pub metadata: ExtraMetadata,          // optional key/value metadata map
    pub submitter: PublicKey,             // signing key of caller
    pub signature: Signature,             // canonical signature over request
}

pub enum BlobClass {
    TaikaiSegment,
    NexusLaneSidecar,
    GovernanceArtifact,
    Custom(u16),
}

pub struct ErasureProfile {
    pub data_shards: u16,
    pub parity_shards: u16,
    pub chunk_alignment: u16, // chunks per availability slice
    pub fec_scheme: FecScheme,
}

pub struct RetentionPolicy {
    pub hot_retention_secs: u64,
    pub cold_retention_secs: u64,
    pub required_replicas: u16,
    pub storage_class: StorageClass,
    pub governance_tag: GovernanceTag,
}

pub struct ExtraMetadata {
    pub items: Vec<MetadataEntry>,
}

pub struct MetadataEntry {
    pub key: String,
    pub value: Vec<u8>,
    pub visibility: MetadataVisibility, // public vs governance-only
}

pub enum MetadataVisibility {
    Public,
    GovernanceOnly,
}

pub struct DaIngestReceipt {
    pub client_blob_id: BlobDigest,
    pub lane_id: LaneId,
    pub epoch: u64,
    pub blob_hash: BlobDigest,          // BLAKE3 of raw payload
    pub chunk_root: BlobDigest,         // Merkle root after chunking
    pub manifest_hash: BlobDigest,      // Norito manifest hash
    pub storage_ticket: StorageTicketId,
    pub pdp_commitment: Option<Vec<u8>>,     // Norito-encoded PDP bytes
    #[norito(default)]
    pub stripe_layout: DaStripeLayout,   // total_stripes, shards_per_stripe, row_parity_stripes
    pub queued_at_unix: u64,
    #[norito(default)]
    pub rent_quote: DaRentQuote,        // XOR rent + incentives derived from policy
    pub operator_signature: Signature,
}
```

> Орындау туралы ескертпе: осы пайдалы жүктемелерге арналған канондық Rust нұсқалары қазір жұмыс істейді
> `iroha_data_model::da::types`, сұрау/түбіртек орауыштары `iroha_data_model::da::ingest`
> және `iroha_data_model::da::manifest` ішіндегі манифест құрылымы.

`compression` өрісі қоңырау шалушылардың пайдалы жүктемені қалай дайындағанын жарнамалайды. Torii қабылдайды
`identity`, `gzip`, `deflate` және `zstd`, бұрын байттарды мөлдір түрде ашады.
қосымша манифесттерді хэштеу, бөлшектеу және тексеру.

### Валидацияны тексеру парағы

1. Norito сұрауының `DaIngestRequest` сәйкестігін тексеріңіз.
2. `total_size` канондық (қысылмаған) пайдалы жүктеме ұзындығынан өзгеше болса немесе конфигурацияланған макс.
3. `chunk_size` туралауын (екіден қуат, <= 2 МБ) орындаңыз.
4. `data_shards + parity_shards` <= жаһандық максимум және паритет >= 2 екеніне көз жеткізіңіз.
5. `retention_policy.required_replica_count` басқарудың бастапқы деңгейін құрметтеу керек.
6. Канондық хэшке қарсы қолтаңбаны тексеру (қолтаңба өрісін қоспағанда).
7. Пайдалы жүктеме хэш + метадеректер бірдей болмаса, `client_blob_id` көшірмелерін қабылдамаңыз.
8. `norito_manifest` берілгенде, схема + хэш сәйкестіктерінің қайта есептелгенін тексеріңіз
   бөлшектенгеннен кейін манифест; әйтпесе түйін манифест жасайды және оны сақтайды.
9. Конфигурацияланған репликация саясатын орындау: Torii жіберілгенді қайта жазады
   `RetentionPolicy` `torii.da_ingest.replication_policy` бар (қараңыз
   `replication-policy.md`) және сақталатын алдын ала жасалған манифесттерді қабылдамайды
   метадеректер мәжбүрленген профильге сәйкес келмейді.

### Бөлшектеу және репликация ағыны

1. Пайдалы жүктемені `chunk_size` ішіне жүктеп алыңыз, әр бөлікке BLAKE3 + Merkle түбірін есептеңіз.
2. Бөлшектік міндеттемелерді (рөл/топ_идентификаторы) түсіретін Norito `DaManifestV1` (жаңа құрылым) құрастырыңыз,
   орналасуын өшіру (жолдар мен бағандар паритеттері мен `ipa_commitment`), сақтау саясаты,
   және метадеректер.
3. `config.da_ingest.manifest_store_dir` астында канондық манифест байттарын кезекке қойыңыз
   (Torii жолақ/дәуір/тізбегі/билет/саусақ ізі арқылы кілттелген `manifest.encoded` файлдарын жазады) сондықтан SoraFS
   Оркестрация оларды қабылдап, сақтау билетін тұрақты деректермен байланыстыра алады.
4. Басқару тегі + саясаты бар `sorafs_car::PinIntent` арқылы PIN ниеттерін жариялаңыз.
5. Norito оқиғасын шығарыңыз `DaIngestPublished` бақылаушыларды (жеңіл клиенттер,
   басқару, аналитика).
6. Қоңырау шалушыға `DaIngestReceipt` қайтарыңыз (Torii DA қызмет кілтімен қол қойылған) және
   `Sora-PDP-Commitment` тақырыбы, сондықтан SDK кодталған міндеттемені бірден түсіре алады. Түбіртек
   енді `rent_quote` (Norito `DaRentQuote`) және `stripe_layout`, жіберушілерді көрсетуге мүмкіндік береді
   негізгі жалдау, резервтік үлес, PDP/PoTR бонус күтулері және 2D өшіру схемасы
   ақшаны бермес бұрын сақтау билеті.

## Сақтау / Тіркеу жаңартулары

- Детерминирленген талдауды қоса отырып, `sorafs_manifest` параметрін `DaManifestV1` көмегімен кеңейтіңіз.
- Нұсқаланған пайдалы жүктеме сілтемесі бар `da.pin_intent` жаңа тізілім ағынын қосыңыз
  манифест хэш + билет идентификаторы.
- Қабылдау кідірісін, бөлшектеу өткізу қабілетін бақылау үшін бақылау құбырларын жаңартыңыз,
  репликацияның артта қалуы және сәтсіздіктер саны.

## Тестілеу стратегиясы

- Схеманы тексеру, қолтаңбаны тексеру, қайталауды анықтау үшін бірлік сынақтары.
- `DaIngestRequest`, манифест және түбіртек Norito кодтауын растайтын алтын сынақтар.
- SoraFS тізілімін айналдыратын интеграциялық қосқыш, түйін + түйреуіш ағындарын бекітеді.
- Кездейсоқ өшіру профильдері мен сақтау комбинацияларын қамтитын сипат сынақтары.
- Дұрыс емес метадеректерден қорғау үшін Norito пайдалы жүктемелерінің бұлдырлануы.

## CLI және SDK құралдары (DA-8)- `iroha app da submit` (жаңа CLI кіру нүктесі) енді операторлар үшін ортақ ингест құрастырушыны/баспагерді қосады
  Taikai бума ағынынан тыс ерікті бөртпелерді қабылдай алады. Команда тұрады
  `crates/iroha_cli/src/commands/da.rs:1` және пайдалы жүктемені, өшіру/сақтау профилін және
  CLI көмегімен канондық `DaIngestRequest` қол қоймас бұрын қосымша метадеректер/манифест файлдары
  конфигурациялау кілті. Сәтті жүгірулер `da_request.{norito,json}` және `da_receipt.{norito,json}` астында сақталады.
  `artifacts/da/submission_<timestamp>/` (`--artifact-dir` арқылы қайта анықтау), сондықтан артефактілерді шығаруға болады
  қабылдау кезінде пайдаланылған нақты Norito байттарын жазыңыз.
- Пәрмен әдепкі бойынша `client_blob_id = blake3(payload)`, бірақ арқылы қайта анықтауды қабылдайды
  `--client-blob-id`, метадеректер JSON карталарын (`--metadata-json`) және алдын ала жасалған манифесттерді құрметтейді
  (`--manifest`) және офлайн дайындау үшін `--no-submit` плюс теңшелетін `--endpoint` қолдайды
  Torii хосттары. JSON түбіртегі дискіге жазылудан басқа, stdout файлында басып шығарылады
  DA-8 "submit_blob" құрал талаптары және SDK паритетінің жұмысын блоктан шығару.
- `iroha app da get` әлдеқашан қуат беретін көп көзді оркестр үшін DA-ға бағытталған бүркеншік атын қосады
  `iroha app sorafs fetch`. Операторлар оны манифестке + кесінді-жоспар артефактілеріне көрсете алады (`--manifest`,
  `--plan`, `--manifest-id`) **немесе** `--storage-ticket` арқылы Torii сақтау билетін өткізіңіз. Билет қашан
  жол пайдаланылады, CLI манифестті `/v1/da/manifests/<ticket>` ішінен шығарады, астындағы топтаманы сақтайды
  `artifacts/da/fetch_<timestamp>/` (`--manifest-cache-dir` көмегімен қайта анықтау), блоб хэшін шығарады
  `--manifest-id`, содан кейін оркестрді берілген `--gateway-provider` тізімімен іске қосады. Барлығы
  SoraFS алу бетіндегі жетілдірілген тұтқалар (манифест конверттері, клиент жапсырмалары, қорғау кэштері,
  анонимдік тасымалдауды қайта анықтау, көрсеткіштер тақтасын экспорттау және `--output` жолдары) және манифесттің соңғы нүктесі мүмкін
  теңшелетін Torii хосттары үшін `--manifest-endpoint` арқылы қайта анықтауға болады, осылайша қол жетімділікті тікелей эфирде тексереді
  Оркестр логикасын қайталаусыз толығымен `da` аттар кеңістігінде.
- `iroha app da get-blob` канондық манифесттерді тікелей Torii-тен `GET /v1/da/manifests/{storage_ticket}` арқылы шығарады.
  Пәрмен `manifest_{ticket}.norito`, `manifest_{ticket}.json` және `chunk_plan_{ticket}.json` деп жазады.
  `artifacts/da/fetch_<timestamp>/` (немесе пайдаланушы берген `--output-dir`) астында дәл қайталау кезінде
  Кейінгі оркестрді алу үшін қажет `iroha app da get` шақыруы (соның ішінде `--manifest-id`).
  Бұл операторларды манифест спуль каталогтарынан сақтайды және алушының әрқашан пайдаланатынына кепілдік береді
  Torii шығарған қол қойылған артефактілер. JavaScript Torii клиенті осы ағынды арқылы көрсетеді
  `ToriiClient.getDaManifest(storageTicketHex)`, декодталған Norito байттарын қайтарады, JSON манифесті,
  және SDK шақырушылары CLI-ге қатыспастан оркестрлік сеанстарды ылғалдата алатындай бөліктік жоспар.
  Swift SDK енді бірдей беттерді көрсетеді (`ToriiClient.getDaManifestBundle(...)` плюс
  `fetchDaPayloadViaGateway(...)`), құбырлар топтамалары жергілікті SoraFS оркестрінің орауышына сәйкес келеді.
  iOS клиенттері манифесттерді жүктей алады, көп дереккөзді алуды орындай алады және дәлелдемелерді түсіре алады
  CLI шақыру.【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240】【IrohaSwift/Sources/IrohaSwift/SorafsOrchestratorClient.swift:12】
- `iroha app da rent-quote` қамтамасыз етілген жад өлшемі үшін детерминирленген жалдау және ынталандыру бұзылыстарын есептейді
  және сақтау терезесі. Көмекші белсенді `DaRentPolicyV1` (JSON немесе Norito байт) немесе тұтынады.
  кірістірілген әдепкі, саясатты тексереді және JSON қорытындысын басып шығарады (`gib`, `months`, саясат метадеректері,
  және `DaRentQuote` өрістері) аудиторлар басқару хаттамаларында нақты XOR төлемдеріне сілтеме жасай алады.
  арнайы сценарийлер жазу. Сондай-ақ пәрмен JSON алдында бір жолды `rent_quote ...` түйіндемесін шығарады
  Оқиға жаттығулары кезінде консоль журналдарын оқуға болатын сақтау үшін пайдалы жүктеме. `--quote-out artifacts/da/rent_quotes/<stamp>.json` арқылы жұптаңыз
  `--policy-label "governance ticket #..."` нақты саясат дауысына сілтеме жасайтын әдемі артефактілерді сақтау үшін
  немесе конфигурациялау жинағы; CLI реттелетін белгіні кесіп тастайды және бос жолдарды қабылдамайды, сондықтан `policy_source` мәндері
  қазынашылық бақылау тақталарында әрекет ету мүмкіндігін сақтайды. Ішкі пәрмен үшін `crates/iroha_cli/src/commands/da.rs` қараңыз
  және саясат схемасы үшін `docs/source/da/rent_policy.md`.【crates/iroha_cli/src/commands/da.rs:1】【docs/source/da/rent_policy.md:1】
- `iroha app da prove-availability` жоғарыда аталғандардың барлығын тізбектейді: ол сақтау билетін алады, жүктеп алады
  канондық манифест жинағы, көп көзді оркестрді (`iroha app sorafs fetch`) қарсы іске қосады.
  берілген `--gateway-provider` тізімі, жүктеп алынған пайдалы жүктемені + таблоның астында сақталады
  `artifacts/da/prove_availability_<timestamp>/` және бар PoR көмекшісін дереу шақырады
  (`iroha app da prove`) алынған байттар арқылы. Операторлар оркестр тұтқаларын реттей алады
  (`--max-peers`, `--scoreboard-out`, манифест соңғы нүктені қайта анықтау) және дәлелдеу үлгісі
  (`--sample-count`, `--leaf-index`, `--sample-seed`) бір пәрмен артефактілерді жасайды
  DA-5/DA-9 аудиттері күтеді: пайдалы жүктеменің көшірмесі, көрсеткіштер тақтасының дәлелдері және JSON дәлелінің қорытындылары.

## TODO шешімінің қысқаша мазмұны

Бұрын блокталған барлық енгізу TODO орындалды және тексерілді:

- **Сығу туралы кеңестер** — Torii қоңырау шалушы ұсынған белгілерді қабылдайды (`identity`, `gzip`, `deflate`,
  `zstd`) және валидация алдында пайдалы жүктемелерді қалыпқа келтіреді, осылайша канондық манифест хэшіне сәйкес келеді.
  қысылған байттар.【crates/iroha_torii/src/da/ingest.rs:220】【crates/iroha_data_model/src/da/types.rs:161】
- **Метадеректерді тек басқару үшін шифрлау** — Torii енді басқару метадеректерін шифрлайды.
  конфигурацияланған ChaCha20-Poly1305 кілті, сәйкес келмейтін белгілерді қабылдамайды және екі айқынды көрсетеді
  конфигурациялау тұтқалары (`torii.da_ingest.governance_metadata_key_hex`,
  `torii.da_ingest.governance_metadata_key_label`) айналуды детерминистік күйде ұстау үшін.【crates/iroha_torii/src/da/ingest.rs:707】【crates/iroha_config/src/parameters/actual.rs:1662】
- **Үлкен пайдалы жүктеме ағыны** — көп бөлікті қабылдау тікелей эфирде. Клиенттер детерминистикалық ағын
  `client_blob_id` пернесі бар `DaIngestChunk` конверттері, Torii әрбір бөлікті тексереді, оларды кезеңге бөледі
  `manifest_store_dir` астында және `is_last` жалауы түскеннен кейін манифестті атомдық түрде қалпына келтіреді,
  бір рет қоңырау шалу арқылы жүктеп салу кезінде байқалатын жедел жадының жоғарылауын жою.【crates/iroha_torii/src/da/ingest.rs:392】
- **Манифест нұсқасы** — `DaManifestV1` анық `version` өрісін қамтиды және Torii бас тартады
  жаңа манифест макеттері жіберілгенде детерминирленген жаңартуларға кепілдік беретін белгісіз нұсқалар.【crates/iroha_data_model/src/da/types.rs:308】
- **PDP/PoTR ілмектері** — PDP міндеттемелері тікелей бөлшектер қоймасынан алынады және сақталады
  манифесттерден басқа, DA-5 жоспарлаушылары канондық деректерден іріктеу тапсырмаларын іске қоса алады және
  `/v1/da/ingest` плюс `/v1/da/manifests/{ticket}` енді `Sora-PDP-Commitment` тақырыбын қамтиды
  base64 Norito пайдалы жүктемесін тасымалдау, сондықтан SDK DA-5 зондтарының нақты міндеттемесін кэштейді. мақсат.【crates/sorafs_car/src/lib.rs:360】【crates/sorafs_manifest/src/pdp.rs:1】【crates/iroha_torii/src/da/ingest.rs:476】

## Іске асыру туралы ескертпелер

- Torii `/v1/da/ingest` соңғы нүктесі енді пайдалы жүктің қысылуын қалыпқа келтіреді, қайта ойнату кэшін күшейтеді,
  канондық байттарды детерминирленген түрде бөледі, `DaManifestV1` қалпына келтіреді, кодталған пайдалы жүктемені түсіреді
  `config.da_ingest.manifest_store_dir` ішіне SoraFS оркестріне арналған және `Sora-PDP-Commitment` қосады
  тақырыбы операторлар PDP жоспарлаушылары сілтеме жасайтын міндеттемені қабылдайды.【crates/iroha_torii/src/da/ingest.rs:220】
- Әрбір қабылданған блок енді `da-commitment-schedule-<lane>-<epoch>-<sequence>-<ticket>.norito` шығарады
  `manifest_store_dir` астындағы жазба канондық `DaCommitmentRecord` мен өңделмеген
  `PdpCommitmentV1` байт, сондықтан DA-3 бума құрастырушылары мен DA-5 жоспарлаушылары бірдей кірістерді гидратациясыз
  манифесттерді қайта оқу немесе жинақтау қоймалары.【crates/iroha_torii/src/da/ingest.rs:1814】
- SDK көмекші API интерфейстері PDP тақырыбының пайдалы жүктемесін шақырушыларды Norito декодтауын қайта орындауға мәжбүрлемей көрсетеді:
  Rust жәшігі `iroha::da::{decode_pdp_commitment_header, receipt_pdp_commitment}`, Python экспорттайды
  `ToriiClient` енді `decode_pdp_commitment_header` және `IrohaSwift` кемелерін қамтиды
  `decodePdpCommitmentHeader` шикі тақырып карталары немесе `HTTPURLResponse` үшін шамадан тыс жүктемелер даналары.【crates/iroha/src/da.rs:1】【python/iroha_torii_client/client.py:1】【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:1】
- Torii сонымен қатар `GET /v1/da/manifests/{storage_ticket}` көрсетеді, осылайша SDK және операторлар манифесттерді ала алады.
  және түйіннің катушка каталогына қол тигізбестен бөлік жоспарларын жасаңыз. Жауап Norito байттарын қайтарады
  (base64), көрсетілген JSON манифесті, `chunk_plan` JSON блобы `sorafs fetch` үшін дайын, сәйкес
  алтылық дайджесттерді (`storage_ticket`, `client_blob_id`, `blob_hash`, `chunk_root`) көрсетеді және
  `Sora-PDP-Commitment` тақырыбы паритет үшін қабылдау жауаптарынан. `block_hash=<hex>` жеткізу
  сұрау жолы детерминирленген `sampling_plan` қайтарады (тағайындау хэші, `sample_window` және үлгіленген
  Толық 2D орналасуын қамтитын `(index, role, group)` кортеждері), сондықтан валидаторлар мен PoR құралдары бірдей сурет салады.
  индекстер.

### Үлкен пайдалы жүктеме ағынының ағыны

Конфигурацияланған жалғыз сұрау шегінен үлкенірек активтерді қабылдауы қажет клиенттер
`POST /v1/da/ingest/chunk/start` қоңырау шалу арқылы ағындық сеанс. Torii a деп жауап береді
`ChunkSessionId` (BLAKE3-сұралған blob метадеректерінен алынған) және келісілген бөлік өлшемі.
Әрбір келесі `DaIngestChunk` сұрауы мыналарды қамтиды:- `client_blob_id` — соңғы `DaIngestRequest` бірдей.
- `chunk_session_id` — кесінділерді іске қосылған сеанспен байланыстырады.
- `chunk_index` және `offset` — детерминирленген тәртіпті қамтамасыз етеді.
- `payload` — келісілген бөлік өлшеміне дейін.
- `payload_hash` — кесіндінің BLAKE3 хэші Torii бүкіл блокты буферлеусіз тексере алады.
- `is_last` — терминал кесіндісін көрсетеді.

Torii `config.da_ingest.manifest_store_dir/chunks/<session>/` астында расталған кесінділерді сақтайды және
идентификатты құрметтеу үшін қайта ойнату кэшінің ішіндегі орындалу барысын жазады. Соңғы бөлік түскенде, Torii
дискідегі пайдалы жүктемені қайта жинайды (жадтың ұлғаюын болдырмау үшін chunk каталогы арқылы ағынмен),
канондық манифест/түбіртек бір реттік жүктеп салулар сияқты есептейді және соңында жауап береді
Кезеңдік артефактты тұтыну арқылы `POST /v1/da/ingest`. Сәтсіз сеанстар анық түрде тоқтатылуы мүмкін немесе
`config.da_ingest.replay_cache_ttl` кейін қоқыс жиналады. Бұл дизайн желі пішімін сақтайды
Norito қолайлы, клиентке арналған қайталанатын хаттамаларды болдырмайды және бар манифест конвейерін қайта пайдаланады
өзгеріссіз.

**Орындау күйі.** Канондық Norito түрлері қазір өмір сүреді
`crates/iroha_data_model/src/da/`:

- `ingest.rs` `DaIngestRequest`/`DaIngestReceipt` анықтайды
  `ExtraMetadata` контейнері Torii арқылы пайдаланылады.【crates/iroha_data_model/src/da/ingest.rs:1】
- `manifest.rs` хосттары `DaManifestV1` және `ChunkCommitment`, Torii кейін шығаратын
  бөлшектеу аяқталды.【crates/iroha_data_model/src/da/manifest.rs:1】
- `types.rs` ортақ бүркеншік аттарды береді (`BlobDigest`, `RetentionPolicy`,
  `ErasureProfile`, т.б.) және төменде құжатталған әдепкі саясат мәндерін кодтайды.【crates/iroha_data_model/src/da/types.rs:240】
- Манифест спул файлдары `config.da_ingest.manifest_store_dir` ішіне түседі, SoraFS оркестріне дайын.
  жадқа кіру үшін бақылаушы.【crates/iroha_torii/src/da/ingest.rs:220】

Сұрау, манифест және түбіртек пайдалы жүктемелері үшін екі жаққа бару қадағаланады
`crates/iroha_data_model/tests/da_ingest_roundtrip.rs`, Norito кодектерін қамтамасыз ету
жаңартулар арасында тұрақты болып қалады.【crates/iroha_data_model/tests/da_ingest_roundtrip.rs:1】

**Сақтау әдепкілері.** Басқару кезінде бастапқы сақтау саясатын ратификациялады
SF-6; `RetentionPolicy::default()` орындайтын әдепкі мәндер:

- ыстық деңгей: 7 күн (`604_800` секунд)
- суық деңгей: 90 күн (`7_776_000` секунд)
- қажетті көшірмелер: `3`
- сақтау класы: `StorageClass::Hot`
- басқару тегі: `"da.default"`

Төменгі ағын операторлары жолақ қабылданған кезде бұл мәндерді анық түрде қайта анықтауы керек
қатаң талаптар.