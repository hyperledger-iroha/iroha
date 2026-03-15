---
lang: uz
direction: ltr
source: docs/source/da/ingest_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1bf79d000e0536da04eafac6c0d896b1bf8f0c454e1bf4c4b97ba22c7c7f5db1
source_last_modified: "2026-01-22T14:35:37.693070+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Sora Nexus Ma'lumotlar mavjudligini qabul qilish rejasi

_Tuzilgan: 2026-02-20 - Egasi: Asosiy protokol WG / Saqlash jamoasi / DA WG_

DA-2 ish oqimi Torii ni Norito chiqaradigan blob ingest API bilan kengaytiradi
metama'lumotlar va urug'lar SoraFS replikatsiyasi. Ushbu hujjat taklif qilingan narsalarni qamrab oladi
sxema, API yuzasi va tekshirish oqimi, shuning uchun amalga oshirishsiz davom etishi mumkin
ajoyib simulyatsiyalarni bloklash (DA-1 kuzatuvlari). Barcha foydali yuk formatlari MUTLAKA
Norito kodeklaridan foydalaning; hech qanday serde/JSON zaxiralariga ruxsat berilmaydi.

## Maqsadlar

- Katta bo'laklarni qabul qiling (Taikai segmentlari, yo'lakchalar, boshqaruv artefaktlari)
  deterministik ravishda Torii dan yuqori.
- Blob, kodek parametrlarini tavsiflovchi kanonik Norito manifestlarini yarating,
  oʻchirish profili va saqlash siyosati.
- SoraFS issiq saqlash va navbatni takrorlash ishlarida parcha metamaʼlumotlarini saqlab qolish.
- SoraFS reestriga va boshqaruviga pin niyatlari + siyosat teglarini nashr qilish
  kuzatuvchilar.
- Qabul kvitansiyalarini oshkor qiling, shunda mijozlar nashrning deterministik isbotini qaytarib oladilar.

## API yuzasi (Torii)

```
POST /v1/da/ingest
Content-Type: application/norito+v1
```

Foydali yuk - Norito kodli `DaIngestRequest`. Javoblardan foydalanish
`application/norito+v1` va `DaIngestReceipt` qaytaring.

| Javob | Ma'nosi |
| --- | --- |
| 202 Qabul qilingan | Blob qismlarga ajratish/ko'paytirish uchun navbatda; kvitansiya qaytarildi. |
| 400 noto'g'ri so'rov | Sxema/hajm buzilishi (tasdiqlash tekshiruvlariga qarang). |
| 401 Ruxsatsiz | API tokeni etishmayotgan/yaroqsiz. |
| 409 Mojaro | `client_blob_id` nusxasi mos kelmaydigan metadata bilan. |
| 413 Yuk juda katta | Blob uzunligi sozlangan chegaradan oshib ketdi. |
| 429 Juda ko'p so'rovlar | Tarif chegarasi. |
| 500 ichki xato | Kutilmagan xatolik (jirga qayd + ogohlantirish). |

```
GET /v1/da/proof_policies
Accept: application/json | application/x-norito
```

Joriy qator katalogidan olingan `DaProofPolicyBundle` versiyasini qaytaradi.
To‘plam `version` (hozirda `1`), `policy_hash` (xesh)ni reklama qiladi.
tartiblangan siyosat roʻyxati) va `policies` yozuvlari `lane_id`, `dataspace_id`,
`alias` va majburiy `proof_scheme` (bugungi kunda `merkle_sha256`; KZG yoʻllari
KZG majburiyatlari mavjud bo'lgunga qadar qabul qilish orqali rad etiladi). Blok sarlavhasi hozir
`da_proof_policies_hash` orqali to'plamni qabul qiladi, shuning uchun mijozlar
DA majburiyatlari yoki dalillarni tekshirishda o'rnatilgan faol siyosat. Ushbu oxirgi nuqtani oling
dalillarni qurishdan oldin ular chiziq siyosati va oqimga mos kelishini ta'minlash uchun
to'plam xesh. Majburiyatlar ro'yxati/tasdiqlash yakuniy nuqtalari bir xil to'plamga ega, shuning uchun SDKlar
isbotni faol siyosat to'plamiga bog'lash uchun qo'shimcha safar kerak emas.

```
GET /v1/da/proof_policy_snapshot
Accept: application/json | application/x-norito
```

`DaProofPolicyBundle` va buyurtma qilingan siyosat roʻyxatini va a. qaytaradi
`policy_hash`, shuning uchun SDKlar blok ishlab chiqarilganda ishlatilgan versiyani mahkamlashi mumkin. The
xesh Norito kodlangan siyosat massivi orqali hisoblanadi va har safar o'zgaradi
Lane-ning `proof_scheme` yangilanadi, bu mijozlarga o'rtasida siljishni aniqlash imkonini beradi.
keshlangan dalillar va zanjir konfiguratsiyasi.

## Taklif etilgan Norito sxemasi

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
```> Amalga oshirish bo'yicha eslatma: ushbu foydali yuklar uchun Rustning kanonik ko'rinishlari hozirda mavjud
> `iroha_data_model::da::types`, `iroha_data_model::da::ingest` da soʻrov/kvitansiya oʻramlari bilan
> va `iroha_data_model::da::manifest` da manifest tuzilmasi.

`compression` maydoni qo'ng'iroq qiluvchilar foydali yukni qanday tayyorlaganliklarini e'lon qiladi. Torii qabul qiladi
`identity`, `gzip`, `deflate` va `zstd`, avval baytlarni shaffof tarzda ochish
ixtiyoriy manifestlarni xeshlash, qismlarga ajratish va tekshirish.

### Tekshirish ro'yxati

1. Norito soʻrovi `DaIngestRequest` sarlavhasiga mos kelishini tekshiring.
2. `total_size` kanonik (siqilgan) foydali yuk uzunligidan farq qilsa yoki konfiguratsiya qilingan maksimal qiymatdan oshsa, bajarilmaydi.
3. `chunk_size` hizalamasini (ikkita quvvat, = 2 ga ishonch hosil qiling.
5. `retention_policy.required_replica_count` boshqaruvning asosini hurmat qilishi kerak.
6. Kanonik xeshga qarshi imzoni tekshirish (imzo maydonidan tashqari).
7. Agar foydali yuk xesh + metamaʼlumotlar bir xil boʻlmasa, `client_blob_id` nusxasini rad eting.
8. `norito_manifest` taqdim etilganda, qayta hisoblangan sxema + xesh mosligini tekshiring
   parchalanishdan keyin namoyon bo'ladi; aks holda tugun manifest hosil qiladi va uni saqlaydi.
9. Sozlangan replikatsiya siyosatini qo'llang: Torii yuborilganlarni qayta yozadi
   `RetentionPolicy` `torii.da_ingest.replication_policy` bilan (qarang.
   `replication_policy.md`) va saqlanishi oldindan tuzilgan manifestlarni rad etadi
   metadata majburiy profilga mos kelmaydi.

### Chiqarish va replikatsiya oqimi1. `chunk_size` ga bo'lak foydali yuk, har bir parcha uchun BLAKE3 + Merkle ildizini hisoblang.
2. Norito `DaManifestV1` (yangi tuzilma) ni yarating, bu esa majburiyatlarni qabul qiladi (rol/guruh_identifikatori),
   tartibini o'chirish (satr va ustunlar pariteti va `ipa_commitment`), saqlash siyosati,
   va metadata.
3. `config.da_ingest.manifest_store_dir` ostida kanonik manifest baytlarini navbatga qo'ying
   (Torii qator/epoch/sequence/chipta/barmoq izi bilan kalitlangan `manifest.encoded` fayllarini yozadi) shuning uchun SoraFS
   orkestr ularni qabul qilishi va saqlash chiptasini doimiy ma'lumotlar bilan bog'lashi mumkin.
4. PIN niyatlarini `sorafs_car::PinIntent` orqali boshqaruv tegi + siyosati bilan nashr eting.
5. Emit Norito hodisasi `DaIngestPublished` kuzatuvchilarni xabardor qilish (engil mijozlar
   boshqaruv, tahlil).
6. `DaIngestReceipt` ni qaytaring (Torii DA xizmat kaliti bilan imzolangan) va qo'shing
   `Sora-PDP-Commitment` javob sarlavhasida base64 Norito kodlash mavjud
   SDKlar zudlik bilan namuna olish urug'ini saqlashi uchun olingan majburiyatning.
   Kvitansiyaga endi `rent_quote` (`DaRentQuote`) va `stripe_layout` kiradi
   Shunday qilib, topshiruvchilar XOR majburiyatlarini, zaxira ulushini, PDP/PoTR bonus kutishlarini,
   va 2D o'chirish matritsasi o'lchamlari bilan birga pul mablag'larini amalga oshirishdan oldin saqlash chiptasi metama'lumotlari.
7. Ixtiyoriy registr metamaʼlumotlari:
   - `da.registry.alias` - umumiy, shifrlanmagan UTF-8 taxallus qatori, ro'yxatga olish kitobi yozuvini kiritish uchun.
   - `da.registry.owner` - ro'yxatga olish kitobiga egalik huquqini yozish uchun ochiq, shifrlanmagan `AccountId` qatori.
   Torii ularni yaratilgan `DaPinIntent`-ga nusxalaydi, shuning uchun quyi oqim pinlarini qayta ishlash taxalluslarni bog'lashi mumkin
   va xom metama'lumotlar xaritasini qayta tahlil qilmasdan egalari; davomida noto'g'ri shakllangan yoki bo'sh qiymatlar rad etiladi
   qabul qilish tekshiruvi.

## Saqlash / Ro'yxatga olish kitobi yangilanishlari

- `sorafs_manifest` ni `DaManifestV1` bilan kengaytirib, deterministik tahlilni yoqing.
- Versiyalangan foydali yuk havolasi bilan `da.pin_intent` yangi registr oqimini qo'shing
  manifest xesh + chipta identifikatori.
- Yutish kechikishini, bo'linish o'tkazuvchanligini kuzatish uchun kuzatuv quvurlarini yangilang,
  replikatsiyalar to'plami va muvaffaqiyatsizliklar soni.
- Torii `/status` javoblari endi eng soʻnggi versiyani koʻrsatadigan `taikai_ingest` massivini oʻz ichiga oladi.
  DA-9ni yoqish uchun kodlovchidan qabul qilish kechikishi, jonli chekka o‘zgarishi va xato hisoblagichlari (klaster, oqim)
  Prometheus ni qirib tashlamasdan to'g'ridan-to'g'ri tugunlardan sog'lom suratlarni olish uchun asboblar paneli.

## Sinov strategiyasi- Sxemani tekshirish, imzolarni tekshirish, dublikatlarni aniqlash uchun birlik testlari.
- `DaIngestRequest`, manifest va kvitansiyaning Norito kodlanishini tasdiqlovchi oltin testlar.
- Integratsiya jabduqlari SoraFS + ro'yxatga olish kitobini o'zgartiradi, chunk + pin oqimlarini tasdiqlaydi.
- Tasodifiy o'chirish profillari va saqlash kombinatsiyalarini o'z ichiga olgan mulk testlari.
- Noto'g'ri shakllangan metama'lumotlardan himoya qilish uchun Norito foydali yuklarining noaniqlanishi.
- Har bir blob sinfi uchun oltin moslamalar ostida yashaydi
  `fixtures/da/ingest/manifests/<blob_class>/manifest.{norito.hex,json}` qo'shimcha qismi bilan
  `fixtures/da/ingest/sample_chunk_records.txt` ro'yxati. E'tibor berilmagan test
  `regenerate_da_ingest_fixtures` armatura yangilaydi, esa
  `manifest_fixtures_cover_all_blob_classes` yangi `BlobClass` varianti qoʻshilishi bilanoq ishlamay qoladi.
  Norito/JSON to'plamini yangilamasdan. Bu DA-2 bo'lganda Torii, SDK va hujjatlarni halol saqlaydi
  yangi blob sirtini qabul qiladi.【fixtures/da/ingest/README.md:1】【crates/iroha_torii/src/da/tests.rs:2902】

## CLI va SDK asboblari (DA-8)- `iroha app da submit` (yangi CLI kirish nuqtasi) endi operatorlar uchun umumiy qabul qiluvchi quruvchi/noshirni oʻrab oladi.
  Taikai to'plami oqimidan tashqarida o'zboshimchalik bilan bloblarni yutishi mumkin. Buyruq yashaydi
  `crates/iroha_cli/src/commands/da.rs:1` va foydali yukni, oʻchirish/saqlash profilini va
  CLI bilan kanonik `DaIngestRequest` imzolashdan oldin ixtiyoriy metadata/manifest fayllari
  konfiguratsiya kaliti. Muvaffaqiyatli yugurishlar `da_request.{norito,json}` va `da_receipt.{norito,json}` ostida davom etadi.
  `artifacts/da/submission_<timestamp>/` (`--artifact-dir` orqali bekor qilish), shuning uchun artefaktlarni chiqarish mumkin
  yutish paytida ishlatiladigan aniq Norito baytlarini yozib oling.
- Buyruq sukut bo'yicha `client_blob_id = blake3(payload)`, lekin orqali bekor qilishni qabul qiladi
  `--client-blob-id`, metadata JSON xaritalarini (`--metadata-json`) va oldindan yaratilgan manifestlarni hurmat qiladi
  (`--manifest`) va oflayn tayyorlanish uchun `--no-submit` va moslashtirilgan `--endpoint` ni qo'llab-quvvatlaydi
  Torii xostlari. JSON kvitansiyasi diskka yozilishidan tashqari stdout-da ham chop etiladi, yopiladi
  DA-8 "submit_blob" asbob talabi va SDK pariteti ishini blokdan chiqarish.
- `iroha app da get` allaqachon quvvatlangan ko'p manbali orkestr uchun DA-ga yo'naltirilgan taxallusni qo'shadi
  `iroha app sorafs fetch`. Operatorlar uni manifest + chunk-plan artefaktlarida ko'rsatishi mumkin (`--manifest`,
  `--plan`, `--manifest-id`) **yoki** oddiygina `--storage-ticket` orqali Torii saqlash chiptasini o'tkazing. Qachon
  chipta yo'li ishlatiladi CLI manifestni `/v1/da/manifests/<ticket>` dan tortib oladi, to'plamni saqlaydi
  `artifacts/da/fetch_<timestamp>/` ostida (`--manifest-cache-dir` bilan bekor qilish), **manifestni chiqaradi
  `--manifest-id` uchun hash** va keyin orkestrni taqdim etilgan `--gateway-provider` bilan boshqaradi
  ro'yxati. Foydali yukni tekshirish hali ham oʻrnatilgan CAR/`blob_hash` dayjestiga tayanadi, shlyuz identifikatori esa
  endi manifest xesh, shuning uchun mijozlar va validatorlar bitta blob identifikatorini almashadilar. Barcha rivojlangan tugmalar
  SoraFS oluvchi yuzasi buzilmagan (manifest konvertlari, mijoz yorliqlari, himoya keshlari, anonimlik transporti)
  bekor qilish, skorbordni eksport qilish va `--output` yo'llari) va manifest so'nggi nuqtasi orqali bekor qilinishi mumkin
  Maxsus Torii xostlari uchun `--manifest-endpoint`, shuning uchun mavjudligini uchdan-end tekshiruvi to'liq nazorat ostida amalga oshiriladi.
  `da` nom maydoni orkestr mantig'ini takrorlamasdan.
- `iroha app da get-blob` kanonik manifestlarni to'g'ridan-to'g'ri Torii dan `GET /v1/da/manifests/{storage_ticket}` orqali tortib oladi.
  Buyruq endi artefaktlarni manifest xesh (blob identifikatori), yozish bilan yorliqlaydi
  `manifest_{manifest_hash}.norito`, `manifest_{manifest_hash}.json` va `chunk_plan_{manifest_hash}.json`
  `artifacts/da/fetch_<timestamp>/` (yoki foydalanuvchi tomonidan taqdim etilgan `--output-dir`) ostida, ayni paytda
  `iroha app da get` chaqiruvi (jumladan, `--manifest-id`) keyingi orkestrni olish uchun zarur.
  Bu operatorlarni manifest spool kataloglaridan saqlaydi va oluvchi har doim dan foydalanishini kafolatlaydi
  Torii tomonidan chiqarilgan imzolangan artefaktlar. JavaScript Torii mijozi bu oqimni orqali aks ettiradi
  `ToriiClient.getDaManifest(storageTicketHex)` esa Swift SDK hozir ochiladi
  `ToriiClient.getDaManifestBundle(...)`. Ikkalasi dekodlangan Norito baytlarini, JSON manifestini, manifest xeshini qaytaradi.SDK qo'ng'iroq qiluvchilar CLI va Swift-ga chiqmasdan orkestr seanslarini namlashi mumkin bo'lgan qismlar rejasi.
  mijozlar qo'shimcha ravishda `fetchDaPayloadViaGateway(...)` raqamiga qo'ng'iroq qilib, ushbu to'plamlarni mahalliy telefon orqali yuborishlari mumkin.
  SoraFS orkestr o'rami.【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240】
- `/v1/da/manifests` javoblari endi `manifest_hash` va ikkala CLI + SDK yordamchilari (`iroha app da get`,
  `ToriiClient.fetchDaPayloadViaGateway` va Swift/JS shlyuz paketlari) bu dayjestni
  o'rnatilgan CAR/blob xeshiga nisbatan foydali yuklarni tekshirishni davom ettirayotganda kanonik manifest identifikatori.
- `iroha app da rent-quote` taqdim etilgan saqlash hajmi uchun deterministik ijara va rag'batlantiruvchi buzilishlarni hisoblaydi
  va saqlash oynasi. Yordamchi faol `DaRentPolicyV1` (JSON yoki Norito bayt) yoki
  o'rnatilgan standart, siyosatni tasdiqlaydi va JSON xulosasini chop etadi (`gib`, `months`, siyosat metama'lumotlari,
  va `DaRentQuote` maydonlari), shuning uchun auditorlar boshqaruv daqiqalarida aniq XOR to'lovlarini keltira oladilar.
  maxsus skriptlarni yozish. Buyruq endi JSONdan oldin bir qatorli `rent_quote ...` xulosasini chiqaradi
  Hodisalar paytida tirnoqlar yaratilganda konsol jurnallari va runbooklarni skanerlashni osonlashtirish uchun foydali yuk.
  `--quote-out artifacts/da/rent_quotes/<stamp>.json` (yoki boshqa yo'l) orqali o'ting
  Chiroyli bosilgan xulosani davom ettirish uchun `--policy-label "governance ticket #..."` dan foydalaning.
  artefakt ma'lum bir ovoz berish/konfiguratsiya to'plamini keltirishi kerak; CLI maxsus teglarni kesadi va bo'sh joyni rad etadi
  dalillar to'plamlarida `policy_source` qiymatlarini mazmunli saqlash uchun satrlar. Qarang
  pastki buyruq uchun `crates/iroha_cli/src/commands/da.rs` va `docs/source/da/rent_policy.md`
  siyosat sxemasi uchun.【crates/iroha_cli/src/commands/da.rs:1】【docs/source/da/rent_policy.md:1】
- PIN ro'yxatga olish kitobi pariteti endi SDK-larga tarqaladi: `ToriiClient.registerSorafsPinManifest(...)`
  JavaScript SDK `iroha app sorafs pin register` tomonidan ishlatiladigan aniq foydali yukni yaratib, kanonik standartlarni qo'llaydi.
  chunker metama'lumotlari, pin siyosatlari, taxallus isbotlari va vorisi dayjestlari
  `/v1/sorafs/pin/register`. Bu CI botlarini va avtomatlashtirishni qachon CLIga o'tishdan saqlaydi
  manifest ro'yxatga olishlarini yozib olish va yordamchi DA-8 uchun TypeScript/README qamrovi bilan yuboriladi.
  Rust/Swift bilan bir qatorda JS da “yuborish/olish/isbotlash” asboblar pariteti to‘liq qondirilgan.【javascript/iroha_js/src/toriiClient.js:1045】【javascript/iroha_js/test/toriiClient.test.js:78】
- `iroha app da prove-availability` yuqoridagilarning barchasini zanjirlaydi: u saqlash chiptasini oladi, yuklab oladi
  kanonik manifest to'plami, ko'p manbali orkestrni (`iroha app sorafs fetch`) qarshi ishlaydi.
  `--gateway-provider` roʻyxati bilan taʼminlangan, yuklab olingan foydali yuk + reyting jadvali ostida saqlanadi
  `artifacts/da/prove_availability_<timestamp>/` va darhol mavjud PoR yordamchisini chaqiradi
  (`iroha app da prove`) olingan baytlar yordamida. Operatorlar orkestr tugmalarini sozlashlari mumkin
  (`--max-peers`, `--scoreboard-out`, manifest oxirgi nuqtani bekor qilish) va isbot namunasi
  (`--sample-count`, `--leaf-index`, `--sample-seed`) esa bitta buyruq artefaktlarni ishlab chiqaradi
  DA-5/DA-9 auditlari tomonidan kutilgan: foydali yuk nusxasi, skorbord dalillari va JSON isboti xulosalari.- `da_reconstruct` (DA-6da yangi) kanonik manifestni va parcha tomonidan chiqarilgan bo'lak katalogini o'qiydi
  saqlaydi (`chunk_{index:05}.bin` tartibi) va tekshirish vaqtida foydali yukni aniq qayta yig'adi
  har bir Blake3 majburiyati. CLI `crates/sorafs_car/src/bin/da_reconstruct.rs` ostida yashaydi va sifatida yuboriladi
  SoraFS asboblar to'plamining bir qismi. Oddiy oqim:
  1. `manifest_<manifest_hash>.norito` va parcha rejasini yuklab olish uchun `iroha app da get-blob --storage-ticket <ticket>`.
  2. `iroha app sorafs fetch --manifest manifest_<manifest_hash>.json --plan chunk_plan_<manifest_hash>.json --output payload.car`
     (yoki `iroha app da prove-availability`, u ostida olib kelish artefaktlari yoziladi
     `artifacts/da/prove_availability_<ts>/`).
  3. `cargo run -p sorafs_car --features cli --bin da_reconstruct --manifest manifest_<manifest_hash>.norito --chunks-dir ./artifacts/da/prove_availability_<ts>/chunks --output reconstructed.bin --json-out summary.json`.

  Regressiya moslamasi `fixtures/da/reconstruct/rs_parity_v1/` ostida ishlaydi va to'liq manifestni oladi
  va `tests::reconstructs_fixture_with_parity_chunks` tomonidan ishlatiladigan parcha matritsasi (ma'lumotlar + paritet). Uni qayta tiklang

  ```sh
  cargo test -p sorafs_car --features da_harness regenerate_da_reconstruct_fixture_assets -- --ignored --nocapture
  ```

  Asbob chiqaradi:

  - `manifest.{norito.hex,json}` — kanonik `DaManifestV1` kodlashlari.
  - `chunk_matrix.json` - hujjat/test havolalari uchun tartiblangan indeks/ofset/uzunlik/dijest/paritet qatorlari.
  - `chunks/` — `chunk_{index:05}.bin` maʼlumotlar va paritet parchalari uchun foydali yuk boʻlaklari.
  - `payload.bin` - paritetdan xabardor jabduqlar testi tomonidan ishlatiladigan deterministik foydali yuk.
  - `commitment_bundle.{json,norito.hex}` — hujjatlar/testlar uchun deterministik KZG majburiyatiga ega `DaCommitmentBundle` namunasi.

  Jabduqlar etishmayotgan yoki kesilgan qismlarni rad etadi, oxirgi foydali yuk Blake3 xeshini `blob_hash` bilan tekshiradi,
  va CI rekonstruksiyani tasdiqlay olishi uchun umumiy JSON blokini chiqaradi (foydali yuk baytlari, bo'laklar soni, saqlash chiptasi)
  dalil. Bu operatorlar va QA deterministik qayta qurish vositasi uchun DA-6 talabini yopadi
  ish o'rinlari maxsus skriptlarni o'tkazmasdan ishga tushishi mumkin.

## TODO qarorining xulosasi

Oldin bloklangan barcha qabul qilish TODOlar amalga oshirildi va tasdiqlandi:- **Siqish boʻyicha maslahatlar** — Torii qoʻngʻiroq qiluvchi tomonidan taqdim etilgan teglarni qabul qiladi (`identity`, `gzip`, `deflate`,
  `zstd`) va tekshirishdan oldin foydali yuklarni normallashtiradi, shuning uchun kanonik manifest xeshi mos keladi.
  dekompressiyalangan baytlar.【crates/iroha_torii/src/da/ingest.rs:220】【crates/iroha_data_model/src/da/types.rs:161】
- **Faqat boshqaruv metama’lumotlarini shifrlash** — Torii endi boshqaruv metama’lumotlarini shifrlaydi
  sozlangan ChaCha20-Poly1305 kaliti, mos kelmaydigan yorliqlarni rad etadi va ikkita aniq ko'rinadi
  konfiguratsiya tugmalari (`torii.da_ingest.governance_metadata_key_hex`,
  `torii.da_ingest.governance_metadata_key_label`) aylanishni deterministik saqlash uchun.【crates/iroha_torii/src/da/ingest.rs:707】【crates/iroha_config/src/parameters/actual.rs:1662】
- **Katta yuk oqimi** — koʻp qismli qabul qilish jonli. Mijozlar deterministik oqim
  `client_blob_id` kalitli `DaIngestChunk` konvertlari, Torii har bir tilimni tasdiqlaydi, ularni bosqichma-bosqich qiladi
  `manifest_store_dir` ostida va `is_last` bayrog'i tushganidan keyin manifestni atomik tarzda qayta tiklaydi,
  bitta qo'ng'iroqni yuklashda ko'rinadigan RAM ko'tarilishini yo'q qilish.【crates/iroha_torii/src/da/ingest.rs:392】
- **Manifest versiyasi** — `DaManifestV1` aniq `version` maydoniga ega va Torii rad etadi
  noma'lum versiyalar, yangi manifest maketlari yuborilganda deterministik yangilanishlarni kafolatlaydi.【crates/iroha_data_model/src/da/types.rs:308】
- **PDP/PoTR ilgaklari** — PDP majburiyatlari to'g'ridan-to'g'ri qismlar do'konidan olinadi va saqlanib qoladi
  manifestlardan tashqari, DA-5 rejalashtiruvchilari kanonik ma'lumotlardan namuna olish muammolarini ishga tushirishlari mumkin; the
  `Sora-PDP-Commitment` sarlavhasi endi `/v1/da/ingest` va `/v1/da/manifests/{ticket}` bilan yetkazib beriladi
  javoblar shuning uchun SDKlar kelajakdagi tekshiruvlar havola qiladigan imzolangan majburiyatni darhol bilib oladi.【crates/sorafs_car/src/lib.rs:360】【crates/sorafs_manifest/src/pdp.rs:1】【crates/iroha_torii/src/da:4/76】rs.
- **Shard kursor jurnali** — chiziqli metamaʼlumotlar `da_shard_id` ni koʻrsatishi mumkin (standart `lane_id`) va
  Sumeragi endi `(shard_id, lane_id)` uchun eng yuqori `(epoch, sequence)`ni saqlab qoladi.
  `da-shard-cursors.norito` DA g'altagi bilan birga qayta ishga tushiriladi, shuning uchun qayta ajratilgan/noma'lum bo'laklarni tashlab, ushlab turing
  deterministik takrorlash. Xotiradagi parcha kursor indeksi endi majburiyatlarni bajara olmayapti
  yo'lak identifikatoriga sukut bo'yicha o'rniga xaritalanmagan yo'llar, kursorni oldinga siljitish va takrorlash xatolarini
  aniq va blok tekshiruvi ajratilgan bilan shard-kursor regressiyalarini rad etadi
  `DaShardCursorViolation` sababi + operatorlar uchun telemetriya belgilari. Ishga tushirish/qo‘lga olish endi DAni to‘xtatadi
  indeks hidratsiyasi, agar Kurada noma'lum chiziq yoki regressiv kursor bo'lsa va qoidabuzarlikni qayd etsa
  blok balandligi, shuning uchun operatorlar DA ga xizmat ko'rsatishdan oldin tuzatishlari mumkin davlat.【crates/iroha_config/src/parameters/actual.rs】【crates/iroha_core/src/da/shard_cursor.rs】【crates/iroha_core/src/ sumeragi/main_loop.rs】【crates/iroha_core/src/state.rs】【crates/iroha_core/src/block.rs】【docs/source/nexus_lanes.md:47】
- **Shard kursorning kechikish telemetriyasi** — `da_shard_cursor_lag_blocks{lane,shard}` o'lchagichi qandayligini bildiradiuzoq shard tasdiqlanayotgan balandlikni kuzatib boradi. Yo'qolgan/eskirgan/noma'lum qatorlar kechikishni o'rnatdi
  talab qilinadigan balandlik (yoki delta) va muvaffaqiyatli avanslar uni nolga qaytaradi, shuning uchun barqaror holat tekis bo'lib qoladi.
  Operatorlar nol bo'lmagan kechikishlar haqida ogohlantirishi kerak, DA g'altagini/jurnalini qoidabuzarlik uchun tekshirishi kerak,
  va blokni o'chirish uchun qayta o'ynashdan oldin tasodifiy qayta ajratish uchun yo'lak katalogini tekshiring
  bo'shliq.
- **Maxfiy hisoblash yo‘llari** — bilan belgilangan yo‘llar
  `metadata.confidential_compute=true` va `confidential_key_version` sifatida qaraladi
  SMPC/shifrlangan DA yo'llari: Sumeragi nol bo'lmagan foydali yuk/manifest dayjestlari va saqlash chiptalarini qo'llaydi,
  to'liq nusxali saqlash profillarini rad etadi va SoraFS chipta + siyosat versiyasini indekssiz qiladi
  foydali yuk baytlarini ko'rsatish. Takrorlash paytida kvitansiyalar Kura dan hidratlanadi, shuning uchun validatorlar xuddi shu narsani tiklaydi
  keyin maxfiylik metadata qayta ishga tushiriladi.【crates/iroha_config/src/parameters/actual.rs】【crates/iroha_core/src/da/confidential.rs】【crates/iroha_core/src/da/confidential_store.rs】【crates/iroha_rs】core.

## Amalga oshirish bo'yicha eslatmalar- Torii ning `/v1/da/ingest` so'nggi nuqtasi endi foydali yukni siqishni normallashtiradi, takrorlash keshini majburlaydi,
  kanonik baytlarni aniq qismlarga ajratadi, `DaManifestV1` ni qayta tiklaydi va kodlangan yukni tushiradi
  kvitansiyani berishdan oldin SoraFS orkestratsiyasi uchun `config.da_ingest.manifest_store_dir` ichiga; the
  ishlov beruvchi, shuningdek, mijozlar kodlangan majburiyatni olishlari uchun `Sora-PDP-Commitment` sarlavhasini biriktiradi.
  darhol.【crates/iroha_torii/src/da/ingest.rs:220】
- Kanonik `DaCommitmentRecord` davom etgandan so'ng, Torii endi
  Manifest spool yonidagi `da-commitment-schedule-<lane>-<epoch>-<sequence>-<ticket>.norito` fayli.
  Har bir yozuv rekordni xom Norito `PdpCommitment` baytlari bilan to'playdi, shuning uchun DA-3 to'plami quruvchilar va
  DA-5 rejalashtiruvchilari manifestlarni qayta oʻqimasdan yoki parcha saqlamasdan bir xil maʼlumotlarni oladi.【crates/iroha_torii/src/da/ingest.rs:1814】
- SDK yordamchilari har bir mijozni Norito tahlilini qayta tiklashga majburlamasdan PDP sarlavhasi baytlarini ochib beradi:
  `iroha::da::{decode_pdp_commitment_header, receipt_pdp_commitment}` qoplamasi Rust, Python `ToriiClient`
  endi `decode_pdp_commitment_header` eksport qiladi va `IrohaSwift` mos keladigan yordamchilarni juda mobil yuboradi
  mijozlar kodlangan namuna olish jadvalini darhol saqlashi mumkin.【crates/iroha/src/da.rs:1】【python/iroha_torii_client/client.py:1】【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:1】
- Torii shuningdek `GET /v1/da/manifests/{storage_ticket}` ni ochib beradi, shuning uchun SDK va operatorlar manifestlarni olishlari mumkin
  va tugunning spool katalogiga tegmasdan parcha rejalari. Javob Norito baytlarini qaytaradi
  (base64), renderlangan manifest JSON, `chunk_plan` JSON blobi `sorafs fetch` uchun tayyor, shuningdek, tegishli
  hex dayjestlar (`storage_ticket`, `client_blob_id`, `blob_hash`, `chunk_root`), shuning uchun quyi oqim asboblari
  Dijestlarni qayta hisoblamasdan orkestrni ta'minlang va bir xil `Sora-PDP-Commitment` sarlavhasini chiqaradi
  ko'zgu yutilish javoblari. `block_hash=<hex>` ni so'rov parametri sifatida o'tkazish deterministikni qaytaradi
  `sampling_plan` ildizi `block_hash || client_blob_id` (validatorlar boʻylab taqsimlanadi) oʻz ichiga olgan
  `assignment_hash`, soʻralgan `sample_window` va namunali `(index, role, group)` kortejlari
  PoR namunalari va validatorlar bir xil indekslarni takrorlashi uchun butun 2D chiziqli tartibi. Namuna oluvchi
  `client_blob_id`, `chunk_root` va `ipa_commitment` ni tayinlash xeshiga aralashtiradi; `iroha ilovasi yuklab olinadi
  --block-hash ` now writes `sampling_plan_.json` manifest + chunk rejasi yonida
  xesh saqlanib qoldi va JS/Swift Torii mijozlari bir xil `assignment_hash_hex` ni ko'rsatadi, shuning uchun validatorlar
  va provers bitta deterministik prob to'plamini baham ko'radi. Torii namuna olish rejasini qaytarganda, `iroha app da
  o'rniga isbot-availability` now reuses that deterministic probe set (seed derived from `sample_seed`).
  vaqtinchalik namuna olish, shuning uchun PoR guvohlari operator topshiriqlarini bajarmagan bo'lsa ham
  `--block-hash` bekor qiling.【crates/iroha_torii_shared/src/da/sampling.rs:1】【crates/iroha_cli/src/commands/da.rs:523】 【Javascript/iroha_js/src/toriiClient.js:15903】【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:170】

### Katta foydali yuk oqimiSozlangan yagona soʻrov chegarasidan kattaroq aktivlarni qabul qilishi kerak boʻlgan mijozlar
`POST /v1/da/ingest/chunk/start` ga qo'ng'iroq qilish orqali oqim seansi. Torii a bilan javob beradi
`ChunkSessionId` (BLAKE3 - so'ralgan blob metama'lumotlaridan olingan) va kelishilgan bo'lak hajmi.
Har bir keyingi `DaIngestChunk` so'rovi quyidagilarni o'z ichiga oladi:

- `client_blob_id` - yakuniy `DaIngestRequest` bilan bir xil.
- `chunk_session_id` - bo'laklarni ishlaydigan seansga bog'laydi.
- `chunk_index` va `offset` - deterministik tartibni amalga oshiradi.
- `payload` - kelishilgan bo'lak hajmigacha.
- `payload_hash` — Torii tilimning BLAKE3 xeshi butun blobni buferlashsiz tekshirishi mumkin.
- `is_last` - terminal bo'lagini bildiradi.

Torii `config.da_ingest.manifest_store_dir/chunks/<session>/` ostida tasdiqlangan boʻlaklarni saqlab qoladi va
identifikatorni hurmat qilish uchun takrorlash keshi ichidagi taraqqiyotni qayd qiladi. Yakuniy bo'lak tushganda, Torii
diskdagi foydali yukni qayta yig'adi (xotiraning keskin o'sishiga yo'l qo'ymaslik uchun chunk katalogi orqali oqim),
kanonik manifest/kvitansiyani bir martalik yuklashlar kabi hisoblab chiqadi va nihoyat javob beradi.
Sahnalashtirilgan artefaktni iste'mol qilish orqali `POST /v1/da/ingest`. Muvaffaqiyatsiz seanslar aniq bekor qilinishi mumkin yoki
`config.da_ingest.replay_cache_ttl` dan keyin axlat yig'iladi. Ushbu dizayn tarmoq formatini saqlaydi
Norito qulay, mijozga xos qayta tiklanadigan protokollardan qochadi va mavjud manifest quvur liniyasidan qayta foydalanadi
o'zgarmagan.

**Imlementatsiya holati.** Norito kanonik turlari hozirda mavjud
`crates/iroha_data_model/src/da/`:

- `ingest.rs` `DaIngestRequest`/`DaIngestReceipt` bilan birga belgilaydi
  Torii tomonidan ishlatiladigan `ExtraMetadata` konteyner.【crates/iroha_data_model/src/da/ingest.rs:1】
- `manifest.rs` xostlari `DaManifestV1` va `ChunkCommitment`, Torii keyin chiqaradi
  parchalanish tugallandi.【crates/iroha_data_model/src/da/manifest.rs:1】
- `types.rs` umumiy taxalluslarni taqdim etadi (`BlobDigest`, `RetentionPolicy`,
  `ErasureProfile` va boshqalar) va quyida hujjatlashtirilgan standart siyosat qiymatlarini kodlaydi.【crates/iroha_data_model/src/da/types.rs:240】
- Manifest spool fayllari `config.da_ingest.manifest_store_dir` ga tushadi, SoraFS orkestratsiyasiga tayyor
  kuzatuvchini saqlashga kirishga torting.【crates/iroha_torii/src/da/ingest.rs:220】
- Sumeragi DA to'plamlarini muhrlash yoki tasdiqlashda manifest mavjudligini ta'minlaydi:
  Agar spoolda manifest yo'q bo'lsa yoki xesh farq qilsa, bloklar tekshiruvdan o'tolmaydi
  majburiyatdan.【crates/iroha_core/src/sumeragi/main_loop.rs:5335】【crates/iroha_core/src/sumeragi/main_loop.rs:14506】

So'rov, manifest va kvitansiya yuklamalari bo'yicha ikki tomonlama qamrovi kuzatiladi
`crates/iroha_data_model/tests/da_ingest_roundtrip.rs`, Norito kodekini ta'minlash
yangilanishlar davomida barqaror qoladi.【crates/iroha_data_model/tests/da_ingest_roundtrip.rs:1】

**Saqlash defoltlari.** Boshqaruv davomida dastlabki saqlash siyosatini ratifikatsiya qildi
SF-6; `RetentionPolicy::default()` tomonidan qo'llaniladigan standart sozlamalar:- issiq daraja: 7 kun (`604_800` soniya)
- sovuq daraja: 90 kun (`7_776_000` soniya)
- kerakli nusxalar: `3`
- saqlash klassi: `StorageClass::Hot`
- boshqaruv yorlig'i: `"da.default"`

Pastki oqim operatorlari chiziq qabul qilinganda bu qiymatlarni aniq bekor qilishi kerak
qattiqroq talablar.

## Mijozning zangdan himoyalangan artefaktlari

Rust mijozini o'rnatgan SDK-lar endi CLI-ga o'tishlari shart emas
kanonik PoR JSON to'plamini ishlab chiqaring. `Client` ikkita yordamchini ko'rsatadi:

- `build_da_proof_artifact` tomonidan yaratilgan aniq tuzilmani qaytaradi
  `iroha app da prove --json-out`, shu jumladan manifest/foydali yuk izohlari
  [`DaProofArtifactMetadata`] orqali.【crates/iroha/src/client.rs:3638】
- `write_da_proof_artifact` quruvchini o'rab oladi va artefaktni diskda saqlaydi
  (sukut bo'yicha chiroyli JSON + keyingi yangi qator), shuning uchun avtomatlashtirish faylni biriktirishi mumkin
  dalillar to'plamini chiqarish yoki boshqarish uchun.【crates/iroha/src/client.rs:3653】

### Misol

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

Yordamchini qoldiradigan JSON foydali yuki CLI dan maydon nomlarigacha mos keladi
(`manifest_path`, `payload_path`, `proofs[*].chunk_digest` va boshqalar), shuning uchun mavjud
avtomatlashtirish faylni formatga oid filiallarsiz farq qilishi/parket qilishi/yuklashi mumkin.

## Tasdiqlash mezonlari

Oldin vakillik yuklari bo'yicha tekshirgich byudjetlarini tasdiqlash uchun DA proof benchmark jabduqlaridan foydalaning
blok darajasidagi qopqoqlarni mahkamlash:

- `cargo xtask da-proof-bench` manifest/foydali yuk juftligidan parchalar do'konini qayta quradi, PoR namunalari
  varaqlar va vaqtni tuzilgan byudjetga nisbatan tekshirish. Taikai metama'lumotlari avtomatik ravishda to'ldiriladi va
  armatura juftligi mos kelmasa, jabduqlar sintetik manifestga qaytadi. Qachon `--payload-bytes`
  aniq `--payload` holda o'rnatiladi, hosil bo'lgan blob quyidagiga yoziladi
  `artifacts/da/proof_bench/payload.bin`, shuning uchun armatura daxlsiz qoladi.【xtask/src/da.rs:1332】【xtask/src/main.rs:2515】
- Hisobotlar standart bo'yicha `artifacts/da/proof_bench/benchmark.{json,md}` bo'lib, isbotlar/yugurish, jami va
  isbotlangan vaqtlar, byudjetdan o'tish tezligi va tavsiya etilgan byudjet (eng sekin iteratsiyaning 110%)
  `zk.halo2.verifier_budget_ms` bilan qatorga chiqing.【artifacts/da/proof_bench/benchmark.md:1】
- Oxirgi ishga tushirish (sintetik 1 Mb foydali yuk, 64 KiB bo'laklar, 32 ta isbot/yugurish, 10 iteratsiya, 250 milodiy byudjet)
  qopqoq ichida 100% iteratsiya bilan 3 ms verifier byudjetini tavsiya qildi.【artifacts/da/proof_bench/benchmark.md:1】
- Misol (deterministik foydali yuk hosil qiladi va ikkala hisobotni yozadi):

```shell
cargo xtask da-proof-bench \
  --payload-bytes 1048576 \
  --sample-count 32 \
  --iterations 10 \
  --budget-ms 250 \
  --json-out artifacts/da/proof_bench/benchmark.json \
  --markdown-out artifacts/da/proof_bench/benchmark.md
```

Bloklarni yig'ish bir xil byudjetlarni qo'llaydi: `sumeragi.da_max_commitments_per_block` va
`sumeragi.da_max_proof_openings_per_block` DA to'plamini blokga joylashtirishdan oldin o'rnatadi va
har bir majburiyatda nolga teng bo'lmagan `proof_digest` bo'lishi kerak. Qo'riqchi to'plam uzunligini o'z ichiga oladi
aniq isbot xulosalari konsensus orqali o'tkazilgunga qadar isbot-ochish sanab, saqlab
≤128 - blok chegarasida amalga oshirilishi mumkin bo'lgan ochilish maqsadi.【crates/iroha_core/src/sumeragi/main_loop.rs:6573】

## PoR xatosi bilan ishlash va kesishSaqlash xodimlari endi PoR xatolik chiziqlarini va har birining yonida bog'langan slash tavsiyalarini ko'rsatadilar
hukm. Sozlangan ogohlantirish chegarasidan yuqori bo'lgan ketma-ket nosozliklar tavsiya qiladi
provayder/manifest juftligi, chiziq chizig‘ini qo‘zg‘atgan chiziq uzunligi va taklif qilingan
provayder obligatsiyasi va `penalty_bond_bps` hisobidan hisoblangan jarima; sovutish oynalari (sekundlar) saqlanadi
Xuddi shu hodisa bo'yicha o'q otishdan ikki nusxadagi qiyshiq chiziqlar.【crates/sorafs_node/src/lib.rs:486】【crates/sorafs_node/src/config.rs:89】【crates/sorafs_node/src/bin/sorafs-node.rs】34:

- Saqlash ishchisi yaratuvchisi orqali chegaralarni/sovutish vaqtini sozlang (birlamchi sozlamalar boshqaruvni aks ettiradi
  jarima siyosati).
- Slash tavsiyalari JSON hukmining xulosasida qayd etilgan, shuning uchun boshqaruv/auditorlar ilova qilishi mumkin
  ularni dalil to'plamlariga.
- Chiziqli joylashuv + har bir bo'lak rollari endi Torii saqlash pinining so'nggi nuqtasi orqali uzatiladi
  (`stripe_layout` + `chunk_roles` maydonlari) va saqlash ishchisida shunday davom etdi
  Auditorlar/ta'mirlash asboblari yuqori oqimdan tartibni qaytadan chiqarmasdan qator/ustunlarni ta'mirlashni rejalashtirishi mumkin

### Joylashtirish + ta'mirlash jabduqlar

`cargo run -p sorafs_car --bin da_reconstruct -- --manifest <path> --chunks-dir <dir>` hozir
`(index, role, stripe/column, offsets)` orqali joylashtirish xeshini hisoblab chiqadi va birinchi qatorni keyin bajaradi
Foydali yukni qayta qurishdan oldin RS(16) ustunini ta'mirlash:

- Joylashuv mavjud bo'lganda sukut bo'yicha `total_stripes`/`shards_per_stripe` ga o'rnatiladi va yana bo'lakka tushadi
- etishmayotgan/buzilgan qismlar birinchi navbatda satr pariteti bilan tiklanadi; bilan qolgan bo'shliqlar tuzatiladi
  chiziqli (ustun) pariteti. Qayta tiklangan qismlar chunk katalogiga va JSONga qayta yoziladi
  Xulosa joylashtirish xeshini va qator/ustunni tuzatish hisoblagichlarini ushlaydi.
- Agar satr+ustun pariteti etishmayotgan to'plamni qondira olmasa, jabduqlar tiklanmaydigan to'plam bilan tezda ishdan chiqadi.
  Auditorlar tuzatib bo'lmaydigan manifestlarni belgilashlari uchun indekslar.