---
lang: kk
direction: ltr
source: docs/source/da/ingest_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1bf79d000e0536da04eafac6c0d896b1bf8f0c454e1bf4c4b97ba22c7c7f5db1
source_last_modified: "2026-01-22T14:35:37.693070+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Sora Nexus Деректер қолжетімділігін қабылдау жоспары

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

```
GET /v1/da/proof_policies
Accept: application/json | application/x-norito
```

Ағымдағы жолақ каталогынан алынған `DaProofPolicyBundle` нұсқасын қайтарады.
Бума `version` (қазіргі `1`), `policy_hash` (хэш) жарнамалайды.
реттелген саясат тізімі) және `policies` жазбалары бар `lane_id`, `dataspace_id`,
`alias` және мәжбүрлі `proof_scheme` (бүгін `merkle_sha256`; KZG жолақтары
KZG міндеттемелері қол жетімді болғанша қабылдау арқылы қабылданбады). Енді блок тақырыбы
пакетке `da_proof_policies_hash` арқылы міндеттенеді, осылайша клиенттер
DA міндеттемелерін немесе дәлелдемелерін тексеру кезінде орнатылған белсенді саясат. Осы соңғы нүктені алыңыз
жолақ саясаты мен ағымға сәйкес келетініне көз жеткізу үшін дәлелдемелерді жасамас бұрын
бума хэш. Міндеттемелер тізімі/дәлелдеу соңғы нүктелері SDK үшін бірдей топтаманы қамтиды
белсенді саясат жинағына дәлелді байланыстыру үшін қосымша бару қажет емес.

```
GET /v1/da/proof_policy_snapshot
Accept: application/json | application/x-norito
```

Тапсырылған саясат тізімін қоса алғанда, `DaProofPolicyBundle` қайтарады
`policy_hash`, сондықтан SDK блок жасалған кезде пайдаланылған нұсқаны бекітеді. The
хэш Norito кодталған саясат массиві арқылы есептеледі және әр кезде өзгереді
Lane `proof_scheme` жаңартылды, бұл клиенттерге олардың арасындағы дрейфті анықтауға мүмкіндік береді.
кэштелген дәлелдер және тізбек конфигурациясы.

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
```> Орындау туралы ескертпе: осы пайдалы жүктемелерге арналған канондық Rust нұсқалары қазір жұмыс істейді
> `iroha_data_model::da::types`, сұрау/түбіртек орауыштары `iroha_data_model::da::ingest`
> және `iroha_data_model::da::manifest` ішіндегі манифест құрылымы.

`compression` өрісі қоңырау шалушылардың пайдалы жүктемені қалай дайындағанын жарнамалайды. Torii қабылдайды
`identity`, `gzip`, `deflate` және `zstd`, бұрын байттарды ашық түрде ашады.
қосымша манифесттерді хэштеу, бөлшектеу және тексеру.

### Валидацияны тексеру парағы

1. Norito сұрауының `DaIngestRequest` сәйкестігін тексеріңіз.
2. `total_size` канондық (қысылмаған) пайдалы жүктеме ұзындығынан өзгеше болса немесе конфигурацияланған макс.
3. `chunk_size` туралауын (екіден қуат, = 2 екеніне көз жеткізіңіз.
5. `retention_policy.required_replica_count` басқарудың бастапқы деңгейін құрметтеу керек.
6. Канондық хэшке қарсы қолтаңбаны тексеру (қолтаңба өрісін қоспағанда).
7. Пайдалы жүктеме хэш + метадеректер бірдей болмаса, `client_blob_id` көшірмелерін қабылдамаңыз.
8. `norito_manifest` берілгенде, схема + хэш сәйкестіктерінің қайта есептелгенін тексеріңіз
   бөлшектенгеннен кейін манифест; әйтпесе түйін манифест жасайды және оны сақтайды.
9. Конфигурацияланған репликация саясатын орындау: Torii жіберілгенді қайта жазады
   `RetentionPolicy` `torii.da_ingest.replication_policy` бар (қараңыз
   `replication_policy.md`) және сақталатын алдын ала жасалған манифесттерді қабылдамайды
   метадеректер мәжбүрленген профильге сәйкес келмейді.

### Бөлшектеу және репликация ағыны1. Пайдалы жүктемені `chunk_size` ішіне жүктеп алыңыз, әр бөлікке BLAKE3 + Merkle түбірін есептеңіз.
2. Бөлшектік міндеттемелерді (рөл/топ_идентификаторы) түсіретін Norito `DaManifestV1` (жаңа құрылым) құрастырыңыз,
   орналасуын өшіру (жолдар мен бағандар паритеттері мен `ipa_commitment`), сақтау саясаты,
   және метадеректер.
3. `config.da_ingest.manifest_store_dir` астында канондық манифест байттарын кезекке қойыңыз
   (Torii жол/дәуір/тізбегі/билет/саусақ ізі арқылы кілттелген `manifest.encoded` файлдарын жазады) сондықтан SoraFS
   Оркестрация оларды қабылдап, сақтау билетін тұрақты деректермен байланыстыра алады.
4. Басқару тегі + саясаты бар `sorafs_car::PinIntent` арқылы PIN ниеттерін жариялаңыз.
5. Norito оқиғасын шығарыңыз `DaIngestPublished` бақылаушыларды (жеңіл клиенттер,
   басқару, аналитика).
6. `DaIngestReceipt` қайтарыңыз (Torii DA қызмет кілтімен қол қойылған) және
   base64 Norito кодтауын қамтитын `Sora-PDP-Commitment` жауап тақырыбы
   SDK-лар сынама тұқымын дереу сақтай алатындай, алынған міндеттеме.
   Түбіртек енді `rent_quote` (`DaRentQuote`) және `stripe_layout` кірістіреді
   сондықтан жіберушілер XOR міндеттемелерін, резервтік үлесті, PDP/PoTR бонустық үміттерін көрсете алады,
   және 2D өшіру матрицасы өлшемдері ақшаны жасамас бұрын сақтау билетінің метадеректерімен бірге.
7. Қосымша тізілім метадеректері:
   - `da.registry.alias` — PIN тізілімінің жазбасын енгізу үшін ашық, шифрланбаған UTF-8 бүркеншік ат жолы.
   - `da.registry.owner` — тізілім иелігін жазуға арналған ашық, шифрланбаған `AccountId` жолы.
   Torii оларды жасалған `DaPinIntent` ішіне көшіреді, сондықтан төменгі пин өңдеу бүркеншік аттарды байланыстыруы мүмкін
   және шикі метадеректер картасын қайта талдаусыз иелері; кезінде дұрыс емес немесе бос мәндер қабылданбайды
   қабылдау валидациясы.

## Сақтау / Тіркеу жаңартулары

- `sorafs_manifest` параметрін `DaManifestV1` көмегімен кеңейтіп, детерминирленген талдауды қосыңыз.
- Нұсқаланған пайдалы жүктеме сілтемесі бар `da.pin_intent` жаңа тізілім ағынын қосыңыз
  манифест хэш + билет идентификаторы.
- Қабылдау кідірісін, бөлшектеу өткізу қабілетін бақылау үшін бақылау құбырларын жаңартыңыз,
  репликацияның артта қалуы және сәтсіздіктер саны.
- Torii `/status` жауаптары енді соңғы нұсқасын көрсететін `taikai_ingest` массивін қамтиды.
  DA-9 мүмкіндігін қоса отырып, кодтауыштан қабылдау кідірісі, тікелей жиектен ауытқу және қате есептегіштері (кластер, ағын)
  бақылау тақталары Prometheus қиып алмастан денсаулық суреттерін тікелей түйіндерден қабылдауға арналған.

## Тестілеу стратегиясы- Схеманы тексеру, қолтаңбаны тексеру, қайталауды анықтау үшін бірлік сынақтары.
- `DaIngestRequest`, манифест және түбіртек Norito кодтауын растайтын алтын сынақтар.
- SoraFS тізілімін айналдыратын интеграциялық қосқыш, түйін + түйреуіш ағындарын бекітеді.
- Кездейсоқ өшіру профильдері мен сақтау комбинацияларын қамтитын сипат сынақтары.
- дұрыс емес метадеректерден қорғау үшін Norito пайдалы жүктемелерінің бұлдырлануы.
- Әрбір блоб класына арналған алтын қондырғылар астында тұрады
  Қосымша бөлігі бар `fixtures/da/ingest/manifests/<blob_class>/manifest.{norito.hex,json}`
  листинг `fixtures/da/ingest/sample_chunk_records.txt`. Еленбеген сынақ
  `regenerate_da_ingest_fixtures` құрылғыларды жаңартады, ал
  `manifest_fixtures_cover_all_blob_classes` жаңа `BlobClass` нұсқасы қосылған бойда сәтсіз аяқталады
  Norito/JSON бумасын жаңартусыз. Бұл DA-2 кез келген уақытта Torii, SDK және құжаттардың адалдығын сақтайды.
  жаңа блоб бетін қабылдайды.【fixtures/da/ingest/README.md:1】【crates/iroha_torii/src/da/tests.rs:2902】

## CLI және SDK құралдары (DA-8)- `iroha app da submit` (жаңа CLI кіру нүктесі) енді операторлар үшін ортақ ингест құрастырушыны/баспагерді қосады
  Taikai бума ағынынан тыс ерікті бөртпелерді қабылдай алады. Команда тұрады
  `crates/iroha_cli/src/commands/da.rs:1` және пайдалы жүктемені, өшіру/сақтау профилін және
  CLI көмегімен канондық `DaIngestRequest` қол қоймас бұрын қосымша метадеректер/манифест файлдары
  конфигурациялау кілті. Сәтті жүгірулер `da_request.{norito,json}` және `da_receipt.{norito,json}` астында сақталады.
  `artifacts/da/submission_<timestamp>/` (`--artifact-dir` арқылы қайта анықтау), сондықтан артефактілерді шығаруға болады
  қабылдау кезінде пайдаланылған нақты Norito байттарын жазыңыз.
- Пәрмен әдепкі бойынша `client_blob_id = blake3(payload)`, бірақ арқылы қайта анықтауды қабылдайды
  `--client-blob-id`, метадеректер JSON карталарын (`--metadata-json`) және алдын ала жасалған манифесттерді құрметтейді
  (`--manifest`) және офлайн дайындау үшін `--no-submit` және теңшелетін `--endpoint` қолдайды
  Torii хосттары. JSON түбіртегі дискіге жазылудан басқа, stdout файлында басып шығарылады
  DA-8 "submit_blob" құрал талаптары және SDK паритетінің жұмысын блоктан шығару.
- `iroha app da get` әлдеқашан қуат беретін көп көзді оркестр үшін DA-ға бағытталған бүркеншік атын қосады
  `iroha app sorafs fetch`. Операторлар оны манифестке + кесінді-жоспар артефактілеріне көрсете алады (`--manifest`,
  `--plan`, `--manifest-id`) **немесе** жай ғана `--storage-ticket` арқылы Torii сақтау билетін өткізіңіз. Қашан
  билет жолы пайдаланылады CLI манифестті `/v1/da/manifests/<ticket>` ішінен шығарады, топтаманы сақтайды
  `artifacts/da/fetch_<timestamp>/` астында (`--manifest-cache-dir` арқылы қайта анықтау), **манифест шығарады
  `--manifest-id` үшін хэш**, содан кейін берілген `--gateway-provider` бар оркестрді іске қосады
  тізім. Шлюз идентификаторы болған кезде пайдалы жүктемені тексеру әлі де ендірілген CAR/`blob_hash` дайджестіне сүйенеді.
  енді манифест хэші, сондықтан клиенттер мен валидаторлар бір блоб идентификаторын ортақ пайдаланады. Барлық жетілдірілген түймелер
  SoraFS қабылдаушы беті бүлінбеген (манифест конверттері, клиент жапсырмалары, қорғау кэштері, анонимді тасымалдау
  қайта анықтау, көрсеткіштер тақтасын экспорттау және `--output` жолдары) және манифесттің соңғы нүктесі арқылы қайта анықтауға болады
  `--manifest-endpoint` теңшелетін Torii хосттары үшін, сондықтан қол жетімділікті түпкілікті тексерулер толығымен орындалады.
  Оркестр логикасын қайталаусыз `da` аттар кеңістігі.
- `iroha app da get-blob` канондық манифесттерді тікелей Torii-тен `GET /v1/da/manifests/{storage_ticket}` арқылы шығарады.
  Енді пәрмен манифест хэшімен (блоб идентификаторы), жазумен артефактілерді белгілейді
  `manifest_{manifest_hash}.norito`, `manifest_{manifest_hash}.json` және `chunk_plan_{manifest_hash}.json`
  `artifacts/da/fetch_<timestamp>/` (немесе пайдаланушы берген `--output-dir`) астында дәл қайталау кезінде
  Кейінгі оркестрді алу үшін қажет `iroha app da get` шақыруы (соның ішінде `--manifest-id`).
  Бұл операторларды манифест спуль каталогтарынан сақтайды және алушының әрқашан пайдаланатынына кепілдік береді
  Torii шығарған қол қойылған артефактілер. JavaScript Torii клиенті осы ағынды арқылы көрсетеді
  `ToriiClient.getDaManifest(storageTicketHex)`, ал Swift SDK енді ашылады
  `ToriiClient.getDaManifestBundle(...)`. Екеуі де декодталған Norito байттарын, JSON манифестін, манифест хэшін,және SDK шақырушылары CLI және Swift-ке шықпай-ақ оркестрлік сеанстарды ылғалдандыруы үшін бөлік жоспары
  клиенттер қосымша `fetchDaPayloadViaGateway(...)` қоңырау шала алады, бұл бумаларды жергілікті телефон арқылы өткізуге болады.
  SoraFS оркестр орамы.【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240】
- `/v1/da/manifests` жауаптары енді `manifest_hash` және CLI + SDK көмекшілері (`iroha app da get`,
  `ToriiClient.fetchDaPayloadViaGateway` және Swift/JS шлюз орауыштары) бұл дайджестті
  кірістірілген CAR/blob хэшіне қарсы пайдалы жүктемелерді тексеруді жалғастыру кезінде канондық манифест идентификаторы.
- `iroha app da rent-quote` қамтамасыз етілген жад өлшемі үшін детерминирленген жалдау және ынталандыру бұзылыстарын есептейді
  және сақтау терезесі. Көмекші белсенді `DaRentPolicyV1` (JSON немесе Norito байт) немесе тұтынады.
  кірістірілген әдепкі, саясатты тексереді және JSON қорытындысын басып шығарады (`gib`, `months`, саясат метадеректері,
  және `DaRentQuote` өрістері) аудиторлар басқару хаттамаларында нақты XOR төлемдеріне сілтеме жасай алады.
  арнайы сценарийлер жазу. Пәрмен енді JSON алдында бір жолды `rent_quote ...` жиынын шығарады
  оқиғалар кезінде тырнақшалар жасалған кезде консоль журналдары мен жұмыс кітапшаларын сканерлеуді жеңілдету үшін пайдалы жүктеме.
  `--quote-out artifacts/da/rent_quotes/<stamp>.json` (немесе кез келген басқа жол) арқылы өту
  әдемі басып шығарылған қорытындыны сақтау үшін және `--policy-label "governance ticket #..."` пайдаланыңыз
  артефакт белгілі бір дауыс/конфигурация жинағына сілтеме жасауы керек; CLI теңшелетін белгілерді қиып, бос орындардан бас тартады
  дәлелдер бумаларында `policy_source` мәндерін мәнді сақтау үшін жолдар. Қараңыз
  Ішкі пәрмен үшін `crates/iroha_cli/src/commands/da.rs` және `docs/source/da/rent_policy.md`
  саясат схемасы үшін.【crates/iroha_cli/src/commands/da.rs:1】【docs/source/da/rent_policy.md:1】
- PIN тізілімінің паритеті енді SDK-ға таралады: `ToriiClient.registerSorafsPinManifest(...)`
  JavaScript SDK `iroha app sorafs pin register` пайдаланатын нақты пайдалы жүктемені құрастырып, канондық талаптарды орындайды.
  chunker метадеректері, түйреуіш саясаттары, бүркеншік аттың дәлелдері және мұрагер дайджесттері үшін POST жібермес бұрын
  `/v1/sorafs/pin/register`. Бұл CI боттары мен автоматтандыруды CLI-ге жіберуден сақтайды
  манифест тіркеулерін жазу және көмекші DA-8 үшін TypeScript/README қамтуымен жеткізіледі.
  «жіберу/алу/дәлелдеу» құрал паритеті JS жүйесінде Rust/Swift-пен бірге толығымен қанағаттандырылады.【javascript/iroha_js/src/toriiClient.js:1045】【javascript/iroha_js/test/toriiClient.test.js:78】78
- `iroha app da prove-availability` жоғарыда аталғандардың барлығын тізбектейді: ол сақтау билетін алады, жүктеп алады
  канондық манифест жинағы, көп көзді оркестрді (`iroha app sorafs fetch`) қарсы іске қосады.
  берілген `--gateway-provider` тізімі, жүктеп алынған пайдалы жүктемені + таблоның астында сақталады
  `artifacts/da/prove_availability_<timestamp>/` және бар PoR көмекшісін дереу шақырады
  (`iroha app da prove`) алынған байттарды пайдаланып. Операторлар оркестр тұтқаларын реттей алады
  (`--max-peers`, `--scoreboard-out`, манифест соңғы нүктені қайта анықтау) және дәлелдеу үлгісі
  (`--sample-count`, `--leaf-index`, `--sample-seed`) бір пәрмен артефактілерді жасайды
  DA-5/DA-9 аудиттері күтеді: пайдалы жүктеменің көшірмесі, көрсеткіштер тақтасының дәлелдері және JSON дәлелінің қорытындылары.- `da_reconstruct` (DA-6-дағы жаңа) канондық манифест пен бөлік шығаратын бөлік каталогын оқиды
  сақтайды (`chunk_{index:05}.bin` орналасуы) және тексеру кезінде пайдалы жүктемені анықтаушы түрде қайта жинайды
  әрбір Blake3 міндеттемесі. CLI `crates/sorafs_car/src/bin/da_reconstruct.rs` астында өмір сүреді және келесідей жеткізіледі
  SoraFS құралдар жинағының бөлігі. Әдеттегі ағын:
  1. `manifest_<manifest_hash>.norito` және бөлік жоспарын жүктеп алу үшін `iroha app da get-blob --storage-ticket <ticket>`.
  2. `iroha app sorafs fetch --manifest manifest_<manifest_hash>.json --plan chunk_plan_<manifest_hash>.json --output payload.car`
     (немесе `iroha app da prove-availability`, ол астында алу артефактілерін жазады
     `artifacts/da/prove_availability_<ts>/` және `chunks/` каталогындағы файлдарды сақтайды).
  3. `cargo run -p sorafs_car --features cli --bin da_reconstruct --manifest manifest_<manifest_hash>.norito --chunks-dir ./artifacts/da/prove_availability_<ts>/chunks --output reconstructed.bin --json-out summary.json`.

  Регрессия құрылғысы `fixtures/da/reconstruct/rs_parity_v1/` астында өмір сүреді және толық манифестті түсіреді
  және `tests::reconstructs_fixture_with_parity_chunks` пайдаланатын бөлшек матрицасы (деректер + паритет). Онымен қалпына келтіріңіз

  ```sh
  cargo test -p sorafs_car --features da_harness regenerate_da_reconstruct_fixture_assets -- --ignored --nocapture
  ```

  Арматура шығарады:

  - `manifest.{norito.hex,json}` — канондық `DaManifestV1` кодтаулары.
  - `chunk_matrix.json` — құжат/тестілеу сілтемелері үшін реттелген индекс/офсет/ұзындық/дайджест/паритет жолдары.
  - `chunks/` — `chunk_{index:05}.bin` деректер мен паритет бөліктері үшін пайдалы жүктеме бөліктері.
  - `payload.bin` — тепе-теңдікті анықтау сынағы пайдаланатын детерминирленген пайдалы жүктеме.
  - `commitment_bundle.{json,norito.hex}` — `DaCommitmentBundle` үлгісі, құжаттар/тесттер үшін KZG детерминирленген міндеттемесі бар.

  Жабдық жетіспейтін немесе кесілген бөліктерден бас тартады, соңғы пайдалы жүктеме Blake3 хэшін `blob_hash` сәйкес тексереді,
  және CI қайта құруды растай алатындай, жиынтық JSON blob шығарады (пайдалы жүк байттары, бөліктер саны, сақтау билеті)
  дәлел. Бұл операторлар мен QA детерминирленген қайта құру құралына арналған DA-6 талабын жабады
  тапсырмалар арнайы сценарийлерді сымсыз шақыра алады.

## TODO шешімінің қысқаша мазмұны

Бұрын блокталған барлық енгізу TODO орындалды және тексерілді:- **Сығу туралы кеңестер** — Torii қоңырау шалушы ұсынған белгілерді қабылдайды (`identity`, `gzip`, `deflate`,
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
  манифесттерден басқа, DA-5 жоспарлаушылары канондық деректерден іріктеу тапсырмаларын іске қоса алады; the
  `Sora-PDP-Commitment` тақырыбы енді `/v1/da/ingest` және `/v1/da/manifests/{ticket}` екеуімен бірге жеткізіледі
  жауаптар, сондықтан SDK болашақ зондтар сілтеме жасайтын қол қойылған міндеттемені бірден біледі.【crates/sorafs_car/src/lib.rs:360】【crates/sorafs_manifest/src/pdp.rs:1】【crates/iroha_torii/src/da:4/76】rs.
- **Shard курсор журналы** — жолдық метадеректер `da_shard_id` (әдепкі бойынша `lane_id`) көрсетуі мүмкін және
  Sumeragi енді `(shard_id, lane_id)` үшін ең жоғары `(epoch, sequence)` сақталады.
  `da-shard-cursors.norito` DA катушкасының жанында, осылайша қайта іске қосылған/белгісіз жолақтарды тастап, сақтайды.
  детерминистикалық қайталау. Жад ішіндегі үзінді курсорының индексі енді міндеттемелер бойынша тез орындалмайды
  жолақ идентификаторына әдепкі мән берудің орнына салыстырылмаған жолақтар, курсорды жылжыту және қайта ойнату қателерін жасайды
  айқын және блокты растау бөлінген параметрі бар shard-курсор регрессияларын қабылдамайды
  `DaShardCursorViolation` себебі + операторларға арналған телеметриялық белгілер. Іске қосу/қуып алу енді DA-ны тоқтатады
  индекс гидратация, егер Курада белгісіз жолақ немесе регрессиялық курсор болса және бұзушылықты жазады
  блок биіктігі, осылайша операторлар DA-ға қызмет көрсету алдында түзетеді күй.【crates/iroha_config/src/parameters/actual.rs】【crates/iroha_core/src/da/shard_cursor.rs】【crates/iroha_core/src/ sumeragi/main_loop.rs】【crates/iroha_core/src/state.rs】【crates/iroha_core/src/block.rs】【docs/source/nexus_lanes.md:47】
- **Меңзердің кідіріс телеметриясы** — `da_shard_cursor_lag_blocks{lane,shard}` көрсеткіші қалай екенін хабарлайдыалыс сынық расталатын биіктікке қарай жүреді. Жетіспейтін/ескірген/белгісіз жолақтар кешігуді орнатады
  талап етілетін биіктік (немесе дельта) және сәтті ілгерілетулер оны нөлге қайтарады, осылайша тұрақты күй біркелкі болып қалады.
  Операторлар нөлдік емес кешігулер туралы дабыл қағуы керек, DA катушкасын/журналын бұзу жолағын тексеруі керек,
  және өшіру үшін блокты қайта ойнатпас бұрын кездейсоқ қайта бөлуге жолақ каталогын тексеріңіз
  алшақтық.
- **Құпия есептеу жолақтары** — белгіленген жолақтар
  `metadata.confidential_compute=true` және `confidential_key_version` ретінде қарастырылады
  SMPC/шифрланған DA жолдары: Sumeragi нөлдік емес пайдалы жүктемені/манифест дайджесттерін және сақтау билеттерін қамтамасыз етеді,
  толық көшірме сақтау профильдерін қабылдамайды және SoraFS билетін + саясат нұсқасын жоқ индекстейді.
  пайдалы жүк байттарын көрсету. Қайталау кезінде Курадан алынған түбіртек гидрат болады, сондықтан валидаторлар бірдей қалпына келтіреді
  кейін құпиялылық метадеректері қайта іске қосылады.【crates/iroha_config/src/parameters/actual.rs】【crates/iroha_core/src/da/confidential.rs】【crates/iroha_core/src/da/confidential_store.rs】【crates/iroha_store.rs】】core/iroha_e.

## Іске асыру туралы ескертпелер- Torii `/v1/da/ingest` соңғы нүктесі енді пайдалы жүктің қысылуын қалыпқа келтіреді, қайта ойнату кэшін күшейтеді,
  канондық байттарды анықтаушы түрде бөледі, `DaManifestV1` қалпына келтіреді және кодталған пайдалы жүктемені түсіреді
  түбіртек беру алдында SoraFS оркестрі үшін `config.da_ingest.manifest_store_dir` ішіне; the
  өңдеуші сонымен қатар `Sora-PDP-Commitment` тақырыбын қосады, осылайша клиенттер кодталған міндеттемені қабылдай алады.
  бірден.【crates/iroha_torii/src/da/ingest.rs:220】
- Канондық `DaCommitmentRecord` сақталғаннан кейін, Torii енді
  `da-commitment-schedule-<lane>-<epoch>-<sequence>-<ticket>.norito` файлы манифест спулының жанындағы.
  Әрбір жазба жазбаны шикі Norito `PdpCommitment` байттарымен жинақтайды, осылайша DA-3 бума құрастырушылары және
  DA-5 жоспарлаушылары манифесттерді немесе жинақ қоймаларын қайта оқусыз бірдей кірістерді қабылдайды.【crates/iroha_torii/src/da/ingest.rs:1814】
- SDK көмекшілері PDP тақырып байттарын әрбір клиентті Norito талдауын қайта орындауға мәжбүрлемей көрсетеді:
  `iroha::da::{decode_pdp_commitment_header, receipt_pdp_commitment}` Rust қақпағы, Python `ToriiClient`
  енді `decode_pdp_commitment_header` экспорттайды және `IrohaSwift` сәйкес көмекшілерді соншалықты мобильді етіп жібереді
  клиенттер кодталған іріктеу кестесін дереу сақтай алады.【crates/iroha/src/da.rs:1】【python/iroha_torii_client/client.py:1】【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:1】】
- Torii сонымен қатар `GET /v1/da/manifests/{storage_ticket}` ашады, осылайша SDK және операторлар манифесттерді ала алады
  және түйіннің катушка каталогына қол тигізбестен бөлік жоспарларын жасаңыз. Жауап Norito байттарын қайтарады
  (base64), көрсетілген JSON манифесті, `chunk_plan` JSON блобы `sorafs fetch` үшін дайын, сонымен қатар сәйкес
  алтылық дайджесттер (`storage_ticket`, `client_blob_id`, `blob_hash`, `chunk_root`), сондықтан төменгі ағынды құралдар
  дайджесттерді қайта есептемей оркестрді беріңіз және бірдей `Sora-PDP-Commitment` тақырыбын шығарады
  айна жауаптары. `block_hash=<hex>` сұрау параметрі ретінде өту детерминистиканы береді
  `sampling_plan` түбірі `block_hash || client_blob_id` (валидаторларда ортақ) бар
  `assignment_hash`, сұралған `sample_window` және үлгіленген `(index, role, group)` кортеждері
  PoR үлгілері мен валидаторлары бірдей индекстерді қайталай алатындай етіп бүкіл 2D жолақ орналасуы. Үлгі алушы
  `client_blob_id`, `chunk_root` және `ipa_commitment` тағайындау хэшіне араластырады; `iroha қолданбасын алуға болады
  --block-hash ` now writes `sampling_plan_.json` манифесттің жанындағы + бөлшек жоспары бар
  хэш сақталады және JS/Swift Torii клиенттері бірдей `assignment_hash_hex`-ті көрсетеді, сондықтан валидаторлар
  және дәлелдеушілер бір детерминирленген зерттеу жинағын бөліседі. Torii іріктеу жоспарын қайтарғанда, `iroha қолданбасы да
  дәлелдеу-қолжетімділігі` now reuses that deterministic probe set (seed derived from `sample_seed`) орнына
  PoR куәгерлері валидатор тағайындауларына сәйкес келетіндіктен, оператор бір тапсырманы өткізіп жіберсе де, арнайы іріктеу
  `--block-hash` қайта анықтау.【crates/iroha_torii_shared/src/da/sampling.rs:1】【crates/iroha_cli/src/commands/da.rs:523】 【javascript/iroha_js/src/toriiClient.js:15903】【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:170】

### Үлкен пайдалы жүктеме ағынының ағыныКонфигурацияланған жалғыз сұрау шегінен үлкенірек активтерді қабылдауы қажет клиенттер
`POST /v1/da/ingest/chunk/start` қоңырау шалу арқылы ағындық сеанс. Torii a деп жауап береді
`ChunkSessionId` (BLAKE3-сұралған blob метадеректерінен алынған) және келісілген бөлік өлшемі.
Әрбір келесі `DaIngestChunk` сұрауы мыналарды қамтиды:

- `client_blob_id` — соңғы `DaIngestRequest` бірдей.
- `chunk_session_id` — кесінділерді іске қосылған сеансқа байланыстырады.
- `chunk_index` және `offset` — детерминирленген тәртіпті қамтамасыз етеді.
- `payload` — келісілген бөлік өлшеміне дейін.
- `payload_hash` — кесіндінің BLAKE3 хэші Torii бүкіл блокты буферлеусіз тексере алады.
- `is_last` — терминал бөлігін көрсетеді.

Torii `config.da_ingest.manifest_store_dir/chunks/<session>/` астында расталған кесінділерді сақтайды және
идентификатты құрметтеу үшін қайта ойнату кэшінің ішіндегі орындалу барысын жазады. Соңғы кесінді түскенде, Torii
дискідегі пайдалы жүктемені қайта жинайды (жадтың ұлғаюын болдырмау үшін chunk каталогы арқылы ағынмен),
канондық манифест/түбіртек бір реттік жүктеп салулар сияқты есептейді және соңында жауап береді
Кезеңдік артефактты тұтыну арқылы `POST /v1/da/ingest`. Сәтсіз сеанстар анық түрде тоқтатылуы мүмкін немесе
`config.da_ingest.replay_cache_ttl` кейін қоқыс жиналады. Бұл дизайн желі пішімін сақтайды
Norito қолайлы, клиентке арналған қайталанатын протоколдарды болдырмайды және бар манифест конвейерін қайта пайдаланады
өзгеріссіз.

**Орындау күйі.** Канондық Norito түрлері қазір өмір сүреді
`crates/iroha_data_model/src/da/`:

- `ingest.rs` `DaIngestRequest`/`DaIngestReceipt` анықтайды
  `ExtraMetadata` контейнері Torii арқылы пайдаланылады.【crates/iroha_data_model/src/da/ingest.rs:1】
- `manifest.rs` хосттары `DaManifestV1` және `ChunkCommitment`, Torii кейін шығаратын
  бөлшектеу аяқталды.【crates/iroha_data_model/src/da/manifest.rs:1】
- `types.rs` ортақ бүркеншік аттарды қамтамасыз етеді (`BlobDigest`, `RetentionPolicy`,
  `ErasureProfile`, т.б.) және төменде құжатталған әдепкі саясат мәндерін кодтайды.【crates/iroha_data_model/src/da/types.rs:240】
- Манифест спул файлдары `config.da_ingest.manifest_store_dir` ішіне түседі, SoraFS оркестріне дайын.
  жадқа кіру үшін бақылаушы.【crates/iroha_torii/src/da/ingest.rs:220】
- Sumeragi DA бумаларын пломбалау немесе тексеру кезінде манифест қолжетімділігін қамтамасыз етеді:
  егер спулда манифест жоқ болса немесе хэш басқаша болса, блоктар тексеруден өтпейді
  міндеттемеден.【crates/iroha_core/src/sumeragi/main_loop.rs:5335】【crates/iroha_core/src/sumeragi/main_loop.rs:14506】

Сұрау, манифест және түбіртек пайдалы жүктемелері үшін екі жаққа бару қадағаланады
`crates/iroha_data_model/tests/da_ingest_roundtrip.rs`, Norito кодегін қамтамасыз ету
жаңартулар арасында тұрақты болып қалады.【crates/iroha_data_model/tests/da_ingest_roundtrip.rs:1】

**Сақтау әдепкілері.** Басқару кезінде бастапқы сақтау саясатын ратификациялады
SF-6; `RetentionPolicy::default()` орындайтын әдепкі мәндер:- ыстық деңгей: 7 күн (`604_800` секунд)
- суық деңгей: 90 күн (`7_776_000` секунд)
- қажетті көшірмелер: `3`
- сақтау класы: `StorageClass::Hot`
- басқару тегі: `"da.default"`

Төменгі ағын операторлары жолақ қабылданған кезде бұл мәндерді анық түрде қайта анықтауы керек
қатаң талаптар.

## Клиенттің тотқа қарсы артефактілері

Rust клиентін ендіретін SDK-лар енді CLI-ге шығудың қажеті жоқ
канондық PoR JSON бумасын жасаңыз. `Client` екі көмекшіні көрсетеді:

- `build_da_proof_artifact` арқылы жасалған нақты құрылымды қайтарады
  `iroha app da prove --json-out`, оның ішінде манифест/пайдалы жүктеме аннотациялары берілген
  [`DaProofArtifactMetadata`] арқылы.【crates/iroha/src/client.rs:3638】
- `write_da_proof_artifact` құрастырушыны орап, артефактты дискіге сақтайды
  (әдепкі бойынша әдемі JSON + кейінгі жаңа жол), сондықтан автоматтандыру файлды тіркей алады
  шығарылымдарға немесе дәлелдемелерді басқаруға арналған.【crates/iroha/src/client.rs:3653】

### Мысал

```rust
use iroha::{
    da::{DaProofArtifactMetadata, DaProofConfig},
    Client,
};

let client = Client::new(config);
let manifest = client.get_da_manifest_bundle(storage_ticket)?;
let payload = std::fs::read("artifacts/da/payload.car")?;
let metadata = DaProofArtifactMetadata::new(
    "artifacts/da/manifest.norito",
    "artifacts/da/payload.car",
);

// Build the JSON artefact in-memory.
let artifact = client.build_da_proof_artifact(
    &manifest,
    &payload,
    &DaProofConfig::default(),
    &metadata,
)?;

// Persist it next to other DA artefacts.
client.write_da_proof_artifact(
    &manifest,
    &payload,
    &DaProofConfig::default(),
    &metadata,
    "artifacts/da/proof_summary.json",
    true,
)?;
```

Көмекші қалдыратын JSON пайдалы жүктемесі CLI-ге өріс атауларына дейін сәйкес келеді
(`manifest_path`, `payload_path`, `proofs[*].chunk_digest`, т.б.), сондықтан бар
автоматтандыру файлды пішімге тән тармақтарсыз ажырата/паркет/жүктеп салуы мүмкін.

## Дәлелдеуді тексеру эталоны

Бұрынғы өкілді пайдалы жүктемелер бойынша тексеруші бюджеттерін тексеру үшін DA proof эталондық белгішесін пайдаланыңыз
блок деңгейіндегі қақпақтарды қатайту:

- `cargo xtask da-proof-bench` манифест/пайдалы жүктеме жұбынан, PoR үлгілерінен бөлшек қоймасын қайта құрады
  конфигурацияланған бюджетке қарсы шығу және уақытты тексеру. Taikai метадеректері автоматты түрде толтырылады және
  арматура жұбы сәйкес келмесе, әбзел синтетикалық манифестке оралады. `--payload-bytes` кезінде
  анық `--payload` орнатылады, жасалған блоб келесіге жазылады
  `artifacts/da/proof_bench/payload.bin`, сондықтан қондырғылар қозғалмайды.【xtask/src/da.rs:1332】【xtask/src/main.rs:2515】
- Есептер әдепкі бойынша `artifacts/da/proof_bench/benchmark.{json,md}` және дәлелдемелерді/іске қосуды, жалпы және
  дәлелдеу уақыттары, бюджетті өту жылдамдығы және ұсынылған бюджет (ең баяу итерацияның 110%)
  `zk.halo2.verifier_budget_ms` арқылы түзетіңіз.【artifacts/da/proof_bench/benchmark.md:1】
- Соңғы іске қосу (синтетикалық 1 МБ пайдалы жүктеме, 64 КБ кесек, 32 дәлел/іс, 10 итерация, 250 мс бюджет)
  қақпақ ішінде итерациялардың 100% болатын 3 мс тексеруші бюджетін ұсынды.【artifacts/da/proof_bench/benchmark.md:1】
- Мысал (детерминирленген пайдалы жүктемені жасайды және екі есепті де жазады):

```shell
cargo xtask da-proof-bench \
  --payload-bytes 1048576 \
  --sample-count 32 \
  --iterations 10 \
  --budget-ms 250 \
  --json-out artifacts/da/proof_bench/benchmark.json \
  --markdown-out artifacts/da/proof_bench/benchmark.md
```

Блок жинағы бірдей бюджеттерді орындайды: `sumeragi.da_max_commitments_per_block` және
`sumeragi.da_max_proof_openings_per_block` DA бумасын блокқа ендірілгенге дейін бекітеді және
әрбір міндеттемеде нөлден басқа `proof_digest` болуы керек. Күзетші байлам ұзындығын ретінде қарастырады
нақты дәлелдемелердің қорытындылары консенсус арқылы тізілгенге дейін дәлелдемелерді ашу санау
≤128- блок шекарасында орындалатын ашу мақсаты.【crates/iroha_core/src/sumeragi/main_loop.rs:6573】

## PoR ақауларын өңдеу және кесуСақтау жұмысшылары енді PoR ақаулық жолақтарын және әрқайсысымен бірге байланыстырылған қиғаш ұсыныстарды көрсетеді
үкім. Конфигурацияланған ереуіл шегінен асатын дәйекті сәтсіздіктер келесі ұсынысты береді
провайдерді/манифест жұбын, қиғаш сызықты тудырған жолақ ұзындығын және ұсынылған
провайдер облигациясынан және `penalty_bond_bps` есептелген өсімпұл; салқындату терезелері (секундтар) сақталады
бір оқиғаға атудан қайталанатын қиғаш сызықтар.【crates/sorafs_node/src/lib.rs:486】【crates/sorafs_node/src/config.rs:89】【crates/sorafs_node/src/bin/sorafs-node.rs】34】

- Сақтау жұмысының құрастырушысы арқылы шекті мәндерді/салқындату уақытын конфигурациялаңыз (әдепкілер басқаруды көрсетеді
  айыппұл саясаты).
- Slash ұсыныстары басқару/аудиторлар тіркей алатындай JSON үкімі қорытындысында жазылған.
  оларды дәлелдемелер жинағы.
- Жолақ орналасуы + әр бөлік рөлдері енді Torii сақтау түйреуішінің соңғы нүктесі арқылы өтеді
  (`stripe_layout` + `chunk_roles` өрістері) және сақтау қызметкерінде сақталды.
  аудиторлар/жөндеу құралдары жолды/бағандарды жөндеуді жоғарғы ағыннан орналасуды қайта шығармай-ақ жоспарлай алады.

### Орналастыру + жөндеу әбзелдері

`cargo run -p sorafs_car --bin da_reconstruct -- --manifest <path> --chunks-dir <dir>` қазір
`(index, role, stripe/column, offsets)` бойынша орналастыру хэшін есептейді және алдымен жолды орындайды, содан кейін
RS(16) бағаны пайдалы жүктемені қайта құру алдында жөндеу:

- Орналастыру бар болған кезде әдепкі бойынша `total_stripes`/`shards_per_stripe` болып табылады және бөлікке қайтады
- Жоқ/бұзылған бөліктер алдымен жол паритетімен қайта құрылады; қалған саңылаулар түзетіледі
  жолақ (баған) паритеті. Жөнделген бөліктер chunk каталогына және JSON файлына қайта жазылады
  жиынтық орналастыру хэшін және жолды/бағанды жөндеу есептегіштерін түсіреді.
- Жол+баған паритеті жетіспейтін жиынды қанағаттандыра алмаса, қалпына келтіруге болмайтын қосылыс тез істен шығады.
  аудиторлар түзетілмейтін манифесттерді белгілей алатындай индекстер.