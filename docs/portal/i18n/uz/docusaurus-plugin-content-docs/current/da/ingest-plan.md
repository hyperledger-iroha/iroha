---
lang: uz
direction: ltr
source: docs/portal/docs/da/ingest-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

sarlavha: Ma'lumotlar mavjudligini qabul qilish rejasi
sidebar_label: qabul qilish rejasi
tavsif: Torii blobni qabul qilish uchun sxema, API yuzasi va tekshirish rejasi.
---

::: Eslatma Kanonik manba
:::

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
| 409 Mojaro | `client_blob_id` nusxasi mos kelmaydigan metamaʼlumotlar bilan. |
| 413 Yuk juda katta | Blob uzunligi sozlangan chegaradan oshib ketdi. |
| 429 Juda ko'p so'rovlar | Tarif chegarasi. |
| 500 ichki xato | Kutilmagan xatolik (jirga qayd + ogohlantirish). |

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
```

> Amalga oshirish bo'yicha eslatma: ushbu foydali yuklar uchun Rustning kanonik ko'rinishlari hozirda mavjud
> `iroha_data_model::da::types`, `iroha_data_model::da::ingest` da soʻrov/kvitansiya oʻramlari bilan
> va `iroha_data_model::da::manifest` da manifest tuzilmasi.

`compression` maydoni qo'ng'iroq qiluvchilar foydali yukni qanday tayyorlaganligini e'lon qiladi. Torii qabul qiladi
`identity`, `gzip`, `deflate` va `zstd`, avval baytlarni shaffof tarzda ochish
ixtiyoriy manifestlarni xeshlash, qismlarga ajratish va tekshirish.

### Tekshirish ro'yxati

1. Norito soʻrovi `DaIngestRequest` sarlavhasiga mos kelishini tekshiring.
2. `total_size` kanonik (siqilgan) foydali yuk uzunligidan farq qilsa yoki konfiguratsiya qilingan maksimal qiymatdan oshsa, bajarilmaydi.
3. `chunk_size` hizalamasini (ikki quvvat, <= 2 MiB) qo'llash.
4. `data_shards + parity_shards` <= global maksimal va paritet >= 2 ga ishonch hosil qiling.
5. `retention_policy.required_replica_count` boshqaruvning asosini hurmat qilishi kerak.
6. Kanonik xeshga qarshi imzoni tekshirish (imzo maydonidan tashqari).
7. Agar foydali yuk xesh + metamaʼlumotlar bir xil boʻlmasa, `client_blob_id` nusxasini rad eting.
8. `norito_manifest` taqdim etilganda, qayta hisoblangan sxema + xesh mosligini tekshiring
   parchalanishdan keyin namoyon bo'ladi; aks holda tugun manifest hosil qiladi va uni saqlaydi.
9. Sozlangan replikatsiya siyosatini qo'llang: Torii yuborilganlarni qayta yozadi
   `RetentionPolicy` `torii.da_ingest.replication_policy` bilan (qarang.
   `replication-policy.md`) va saqlanishi oldindan tuzilgan manifestlarni rad etadi
   metadata majburiy profilga mos kelmaydi.

### Chiqarish va replikatsiya oqimi

1. `chunk_size` ga bo'lak foydali yuk, har bir parcha uchun BLAKE3 + Merkle ildizini hisoblang.
2. Norito `DaManifestV1` (yangi tuzilma) ni yarating, bu esa majburiyatlarni qabul qiladi (rol/guruh_identifikatori),
   tartibini o'chirish (satr va ustunlar pariteti va `ipa_commitment`), saqlash siyosati,
   va metadata.
3. `config.da_ingest.manifest_store_dir` ostida kanonik manifest baytlarini navbatga qo'ying
   (Torii qator/epoch/sequence/chipta/barmoq izi bilan kalitlangan `manifest.encoded` fayllarini yozadi) shuning uchun SoraFS
   orkestr ularni qabul qilishi va saqlash chiptasini doimiy ma'lumotlar bilan bog'lashi mumkin.
4. PIN niyatlarini `sorafs_car::PinIntent` orqali boshqaruv tegi + siyosati bilan nashr eting.
5. Kuzatuvchilarni xabardor qilish uchun Norito hodisasi `DaIngestPublished` chiqaring
   boshqaruv, tahlil).
6. `DaIngestReceipt` qo'ng'iroq qiluvchiga qaytaring (Torii DA xizmat kaliti bilan imzolangan) va yuboring
   `Sora-PDP-Commitment` sarlavhasi, shuning uchun SDKlar kodlangan majburiyatni darhol qabul qilishi mumkin. Kvitansiya
   endi `rent_quote` (Norito `DaRentQuote`) va `stripe_layout` o'z ichiga oladi, bu jo'natuvchilarni ko'rsatishga imkon beradi
   asosiy ijara, zaxira ulushi, PDP/PoTR bonus kutishlari va 2D oʻchirish tartibi
   pul mablag'larini topshirishdan oldin saqlash chiptasi.

## Saqlash / Ro'yxatga olish kitobi yangilanishlari

- `sorafs_manifest` ni `DaManifestV1` bilan kengaytirib, deterministik tahlil qilish imkonini beradi.
- Versiyalangan foydali yuk havolasi bilan `da.pin_intent` yangi registr oqimini qo'shing
  manifest xesh + chipta identifikatori.
- Yutish kechikishini, bo'linish o'tkazuvchanligini kuzatish uchun kuzatuv quvurlarini yangilang,
  replikatsiyalar to'plami va muvaffaqiyatsizliklar soni.

## Sinov strategiyasi

- Sxemani tekshirish, imzolarni tekshirish, dublikatlarni aniqlash uchun birlik testlari.
- `DaIngestRequest`, manifest va kvitansiyaning Norito kodlanishini tasdiqlovchi oltin testlar.
- Integratsiya jabduqlari soxta SoraFS + registrni aylantirib, bo'lak + pin oqimlarini tasdiqlaydi.
- Tasodifiy o'chirish profillari va saqlash kombinatsiyalarini o'z ichiga olgan mulk testlari.
- Noto'g'ri shakllangan metama'lumotlardan himoya qilish uchun Norito foydali yuklarining noaniqlanishi.

## CLI va SDK asboblari (DA-8)- `iroha app da submit` (yangi CLI kirish nuqtasi) endi operatorlar uchun umumiy qabul qiluvchi quruvchi/noshirni oʻrab oladi.
  Taikai to'plami oqimidan tashqarida o'zboshimchalik bilan bloblarni yutishi mumkin. Buyruq yashaydi
  `crates/iroha_cli/src/commands/da.rs:1` va foydali yukni, oʻchirish/saqlash profilini va
  CLI bilan kanonik `DaIngestRequest`ni imzolashdan oldin ixtiyoriy metadata/manifest fayllari
  konfiguratsiya kaliti. Muvaffaqiyatli yugurishlar `da_request.{norito,json}` va `da_receipt.{norito,json}` ostida davom etadi.
  `artifacts/da/submission_<timestamp>/` (`--artifact-dir` orqali bekor qilish), shuning uchun artefaktlarni chiqarish mumkin
  yutish paytida ishlatiladigan aniq Norito baytlarini yozib oling.
- Buyruq sukut bo'yicha `client_blob_id = blake3(payload)`, lekin orqali bekor qilishni qabul qiladi
  `--client-blob-id`, metadata JSON xaritalarini (`--metadata-json`) va oldindan yaratilgan manifestlarni hurmat qiladi
  (`--manifest`) va oflayn tayyorlanish uchun `--no-submit` va moslashtirilgan uchun `--endpoint` ni qo'llab-quvvatlaydi
  Torii xostlari. JSON kvitansiyasi diskka yozilishidan tashqari stdout-da ham chop etiladi, yopiladi
  DA-8 "submit_blob" asbob talabi va SDK pariteti ishini blokdan chiqarish.
- `iroha app da get` allaqachon kuchga ega bo'lgan ko'p manbali orkestr uchun DA-ga yo'naltirilgan taxallusni qo'shadi
  `iroha app sorafs fetch`. Operatorlar uni manifest + chunk-plan artefaktlariga ko'rsatishi mumkin (`--manifest`,
  `--plan`, `--manifest-id`) **yoki** `--storage-ticket` orqali Torii saqlash chiptasini o'tkazing. Qachon chipta
  yo'l ishlatiladi, CLI manifestni `/v1/da/manifests/<ticket>` dan tortib oladi, ostidagi to'plamni saqlaydi
  `artifacts/da/fetch_<timestamp>/` (`--manifest-cache-dir` bilan bekor qilish), uchun blob xesh hosil qiladi
  `--manifest-id` va keyin orkestrni taqdim etilgan `--gateway-provider` ro'yxati bilan ishga tushiradi. Hammasi
  SoraFS oluvchi sirtining mukammal tugmalari (manifest konvertlari, mijoz yorliqlari, himoya keshlari,
  anonimlik transportini bekor qilish, skorbordni eksport qilish va `--output` yo'llari) va manifest so'nggi nuqtasi mumkin
  maxsus Torii xostlari uchun `--manifest-endpoint` orqali bekor qilinishi mumkin, shuning uchun mavjudlikni oxirigacha tekshiradi
  to'liq `da` nom maydoni ostida orkestr mantig'ini takrorlamasdan.
- `iroha app da get-blob` kanonik manifestlarni to'g'ridan-to'g'ri Torii dan `GET /v1/da/manifests/{storage_ticket}` orqali tortib oladi.
  Buyruq `manifest_{ticket}.norito`, `manifest_{ticket}.json` va `chunk_plan_{ticket}.json` ni yozadi.
  `artifacts/da/fetch_<timestamp>/` (yoki foydalanuvchi tomonidan taqdim etilgan `--output-dir`) ostida, ayni paytda
  `iroha app da get` chaqiruvi (jumladan, `--manifest-id`) keyingi orkestrni olish uchun zarur.
  Bu operatorlarni manifest spool kataloglaridan saqlaydi va oluvchi har doim dan foydalanishini kafolatlaydi
  Torii tomonidan chiqarilgan imzolangan artefaktlar. JavaScript Torii mijozi bu oqimni orqali aks ettiradi
  `ToriiClient.getDaManifest(storageTicketHex)`, dekodlangan Norito baytlarini qaytaradi, manifest JSON,
  SDK qo'ng'iroq qiluvchilar CLI-ga o'tkazmasdan orkestr seanslarini namlashi mumkin bo'lgan qismlar rejasi.
  Swift SDK endi bir xil sirtlarni ochib beradi (`ToriiClient.getDaManifestBundle(...)` plus
  `fetchDaPayloadViaGateway(...)`), quvur to'plamlari mahalliy SoraFS orkestr o'ramiga o'rnatiladi
  iOS mijozlari manifestlarni yuklab olishlari, ko'p manbali yuklarni bajarishlari va dalillarni olishlari mumkin
  CLI-ni ishga tushirish.【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:240】【IrohaSwift/Sources/IrohaSwift/SorafsOrchestratorClient.swift:12】
- `iroha app da rent-quote` taqdim etilgan saqlash hajmi uchun deterministik ijara va rag'batlantiruvchi buzilishlarni hisoblaydi
  va saqlash oynasi. Yordamchi faol `DaRentPolicyV1` (JSON yoki Norito bayt) yoki
  oʻrnatilgan standart, siyosatni tasdiqlaydi va JSON xulosasini chop etadi (`gib`, `months`, siyosat metamaʼlumotlari,
  va `DaRentQuote` maydonlari), shuning uchun auditorlar boshqaruv daqiqalarida aniq XOR to'lovlarini keltira oladilar.
  maxsus skriptlarni yozish. Buyruq, shuningdek, JSON oldidan bir qatorli `rent_quote ...` xulosasini chiqaradi.
  Hodisa mashqlari paytida konsol jurnallarini o'qish uchun foydali yuk. `--quote-out artifacts/da/rent_quotes/<stamp>.json` bilan ulang
  `--policy-label "governance ticket #..."`, aniq siyosat ovozini keltiruvchi chiroyli artefaktlarni saqlab qolish uchun
  yoki konfiguratsiya to'plami; CLI maxsus yorliqni kesadi va bo'sh satrlarni rad etadi, shuning uchun `policy_source` qiymatlari
  g'aznachilik panellari bo'ylab harakat qilish mumkin. Pastki buyruq uchun `crates/iroha_cli/src/commands/da.rs` ga qarang
  va siyosat sxemasi uchun `docs/source/da/rent_policy.md`.【crates/iroha_cli/src/commands/da.rs:1】【docs/source/da/rent_policy.md:1】
- `iroha app da prove-availability` yuqoridagilarning barchasini zanjirlaydi: u saqlash chiptasini oladi, yuklab oladi
  kanonik manifest to'plami, ko'p manbali orkestratorni (`iroha app sorafs fetch`) qarshi ishlaydi.
  `--gateway-provider` roʻyxati bilan taʼminlangan, yuklab olingan foydali yuk + reyting jadvali ostida saqlanadi
  `artifacts/da/prove_availability_<timestamp>/` va darhol mavjud PoR yordamchisini chaqiradi
  (`iroha app da prove`) olingan baytlar yordamida. Operatorlar orkestr tugmalarini sozlashlari mumkin
  (`--max-peers`, `--scoreboard-out`, manifest oxirgi nuqtani bekor qilish) va isbot namunasi
  (`--sample-count`, `--leaf-index`, `--sample-seed`) esa bitta buyruq artefaktlarni ishlab chiqaradi
  DA-5/DA-9 auditlari tomonidan kutilgan: foydali yuk nusxasi, skorbord dalillari va JSON isboti xulosalari.

## TODO qarorining xulosasi

Oldin bloklangan barcha qabul qilish TODOlar amalga oshirildi va tasdiqlandi:

- **Siqish bo‘yicha maslahatlar** — Torii qo‘ng‘iroq qiluvchi tomonidan taqdim etilgan teglarni qabul qiladi (`identity`, `gzip`, `deflate`,
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
  manifestlardan tashqari, DA-5 rejalashtiruvchilari kanonik ma'lumotlardan namuna olish muammolarini ishga tushirishlari mumkin va
  `/v1/da/ingest` plus `/v1/da/manifests/{ticket}` endi `Sora-PDP-Commitment` sarlavhasini o'z ichiga oladi
  base64 Norito foydali yukni ko'tarib, SDK DA-5 problarining aniq majburiyatlarini keshlaydi. target.【crates/sorafs_car/src/lib.rs:360】【crates/sorafs_manifest/src/pdp.rs:1】【crates/iroha_torii/src/da/ingest.rs:476】

## Amalga oshirish bo'yicha eslatmalar

- Torii ning `/v1/da/ingest` so'nggi nuqtasi endi foydali yukni siqishni normallashtiradi, takrorlash keshini ta'minlaydi,
  kanonik baytlarni aniq qismlarga ajratadi, `DaManifestV1` ni qayta tiklaydi, kodlangan yukni tushiradi
  SoraFS orkestratsiyasi uchun `config.da_ingest.manifest_store_dir` ichiga va `Sora-PDP-Commitment` qo'shadi
  sarlavha, shuning uchun operatorlar PDP rejalashtiruvchilari havola qiladigan majburiyatlarni oladilar.【crates/iroha_torii/src/da/ingest.rs:220】
- Har bir qabul qilingan blob endi `da-commitment-schedule-<lane>-<epoch>-<sequence>-<ticket>.norito` ni ishlab chiqaradi
  kanonik `DaCommitmentRecord` ni xom bilan birga birlashtirgan `manifest_store_dir` ostida kirish
  `PdpCommitmentV1` baytlar, shuning uchun DA-3 to'plami quruvchilari va DA-5 rejalashtiruvchilari bir xil kirishlarni hidratlantirmasdan
  manifestlarni qayta o'qish yoki parchalarni saqlash.【crates/iroha_torii/src/da/ingest.rs:1814】
- SDK yordamchi API'lari qo'ng'iroq qiluvchilarni Norito dekodlashni qayta tiklashga majburlamasdan, PDP sarlavhasining foydali yukini ochib beradi:
  Rust sandiq eksporti `iroha::da::{decode_pdp_commitment_header, receipt_pdp_commitment}`, Python
  `ToriiClient` endi `decode_pdp_commitment_header` va `IrohaSwift` kemalarini o'z ichiga oladi
  Xom sarlavhali xaritalar yoki `HTTPURLResponse` uchun `decodePdpCommitmentHeader` ortiqcha yuklar misollar.【crates/iroha/src/da.rs:1】【python/iroha_torii_client/client.py:1】【IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:1】
- Torii shuningdek `GET /v1/da/manifests/{storage_ticket}` ni ochib beradi, shuning uchun SDK va operatorlar manifestlarni olishlari mumkin
  va tugunning spool katalogiga tegmasdan parcha rejalari. Javob Norito baytlarini qaytaradi
  (base64), renderlangan manifest JSON, `chunk_plan` JSON blobi `sorafs fetch` uchun tayyor, tegishli
  hex hazm qiladi (`storage_ticket`, `client_blob_id`, `blob_hash`, `chunk_root`) va aks ettiradi
  Paritet uchun qabul qilingan javoblardan `Sora-PDP-Commitment` sarlavhasi. `block_hash=<hex>` ni yetkazib berish
  so'rovlar qatori deterministik `sampling_plan` (tayinlash xeshi, `sample_window` va namunali)ni qaytaradi
  `(index, role, group)` to'liq 2D tartibini qamrab oluvchi kortejlar) shuning uchun validatorlar va PoR vositalari bir xil chiziladi
  indekslari.

### Katta foydali yuk oqimi

Sozlangan yagona soʻrov chegarasidan kattaroq aktivlarni qabul qilishi kerak boʻlgan mijozlar
`POST /v1/da/ingest/chunk/start` ga qo'ng'iroq qilish orqali oqim seansi. Torii a bilan javob beradi
`ChunkSessionId` (BLAKE3 - so'ralgan blob metama'lumotlaridan olingan) va kelishilgan bo'lak hajmi.
Har bir keyingi `DaIngestChunk` so'rovi quyidagilarni o'z ichiga oladi:- `client_blob_id` — yakuniy `DaIngestRequest` bilan bir xil.
- `chunk_session_id` - bo'laklarni ishlaydigan seansga bog'laydi.
- `chunk_index` va `offset` - deterministik tartibni amalga oshiradi.
- `payload` - kelishilgan bo'lak hajmigacha.
- `payload_hash` - bo'limning BLAKE3 xeshi, shuning uchun Torii butun blobni buferlamasdan tekshirishi mumkin.
- `is_last` - terminal bo'lagini bildiradi.

Torii `config.da_ingest.manifest_store_dir/chunks/<session>/` ostida tasdiqlangan boʻlaklarni saqlab qoladi va
identifikatorni hurmat qilish uchun takrorlash keshi ichidagi taraqqiyotni qayd qiladi. Yakuniy bo'lak tushganda, Torii
diskdagi foydali yukni qayta yig'adi (xotiraning keskin o'sishiga yo'l qo'ymaslik uchun chunk katalogi orqali oqim),
kanonik manifest/kvitansiyani bir martalik yuklashlar kabi hisoblab chiqadi va nihoyat javob beradi.
Sahnalashtirilgan artefaktni iste'mol qilish orqali `POST /v1/da/ingest`. Muvaffaqiyatsiz seanslar aniq bekor qilinishi mumkin yoki
`config.da_ingest.replay_cache_ttl` dan keyin axlat yig'iladi. Ushbu dizayn tarmoq formatini saqlaydi
Norito qulay, mijozga xos qayta tiklanadigan protokollardan qochadi va mavjud manifest quvur liniyasidan qayta foydalanadi
o'zgarmagan.

**Imlementatsiya holati.** Kanonik Norito turlari hozirda mavjud
`crates/iroha_data_model/src/da/`:

- `ingest.rs` `DaIngestRequest`/`DaIngestReceipt` bilan birga belgilaydi
  Torii tomonidan ishlatiladigan `ExtraMetadata` konteyner.【crates/iroha_data_model/src/da/ingest.rs:1】
- `manifest.rs` xostlari `DaManifestV1` va `ChunkCommitment`, Torii keyin chiqaradi
  parchalanish tugallandi.【crates/iroha_data_model/src/da/manifest.rs:1】
- `types.rs` umumiy taxalluslarni taqdim etadi (`BlobDigest`, `RetentionPolicy`,
  `ErasureProfile` va boshqalar) va quyida hujjatlashtirilgan standart siyosat qiymatlarini kodlaydi.【crates/iroha_data_model/src/da/types.rs:240】
- Manifest spool fayllari `config.da_ingest.manifest_store_dir` ga tushadi, SoraFS orkestratsiyasiga tayyor
  kuzatuvchini saqlashga kirishga torting.【crates/iroha_torii/src/da/ingest.rs:220】

So'rov, manifest va kvitansiya yuklamalari bo'yicha ikki tomonlama qamrovi kuzatiladi
`crates/iroha_data_model/tests/da_ingest_roundtrip.rs`, Norito kodekini ta'minlaydi
yangilanishlar davomida barqaror qoladi.【crates/iroha_data_model/tests/da_ingest_roundtrip.rs:1】

**Saqlash defoltlari.** Boshqaruv davomida dastlabki saqlash siyosatini ratifikatsiya qildi
SF-6; `RetentionPolicy::default()` tomonidan qo'llaniladigan standart sozlamalar:

- issiq daraja: 7 kun (`604_800` soniya)
- sovuq daraja: 90 kun (`7_776_000` soniya)
- kerakli nusxalar: `3`
- saqlash klassi: `StorageClass::Hot`
- boshqaruv yorlig'i: `"da.default"`

Pastki oqim operatorlari chiziq qabul qilinganda bu qiymatlarni aniq bekor qilishi kerak
qattiqroq talablar.