---
lang: uz
direction: ltr
source: docs/portal/docs/sorafs/node-client-protocol.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e0cdd8242b45628e688d94ebec08e2d9900787ec93a81417e6683d399d43be2d
source_last_modified: "2026-01-22T14:35:36.781385+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS tugun ↔ Mijoz protokoli

Ushbu qo'llanmada kanonik protokol ta'rifini umumlashtiradi
[`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md).
Norito bayt darajasidagi sxemalar va o'zgarishlar jurnallari uchun yuqori oqim spetsifikatsiyasidan foydalaning; portal
nusxasi operatsion muhim nuqtalarni qolgan SoraFS runbooks yaqinida saqlaydi.

## Provayder reklamalari va tasdiqlash

SoraFS provayderlari `ProviderAdvertV1` foydali yuklarni g'iybat qilishadi (qarang.
`crates/sorafs_manifest::provider_advert`) boshqaruvchi operator tomonidan imzolangan.
Reklamalar kashfiyot meta-ma'lumotlarini pin qiladi va to'siqlar ko'p manbali
orkestr ish vaqtida amalga oshiradi.

- **Umr bo'yi** — `issued_at < expires_at ≤ issued_at + 86 400 s`. Provayderlar
  har 12 soatda yangilanishi kerak.
- **Capability TLVs** — TLV roʻyxati transport xususiyatlarini reklama qiladi (Torii,
  QUIC+Shovqin, SoraNet relelari, sotuvchi kengaytmalari). Noma'lum kodlar o'tkazib yuborilishi mumkin
  qachon `allow_unknown_capabilities = true`, GREASE ko'rsatmalariga rioya qilgan holda.
- **QoS maslahatlari** — `availability` darajasi (issiq/issiq/sovuq), maksimal olish
  kechikish, parallellik chegarasi va ixtiyoriy oqim byudjeti. QoS bilan mos kelishi kerak
  telemetriya kuzatilgan va qabul qilish orqali tekshiriladi.
- **Yakuniy nuqtalar va uchrashuv mavzulari** — TLS/ALPN bilan aniq xizmat URL manzillari
  metama'lumotlar va mijozlar qurishda obuna bo'lishi kerak bo'lgan kashfiyot mavzulari
  qo'riqlash to'plamlari.
- **Yo‘l xilma-xilligi siyosati** — `min_guard_weight`, AS/hovuzdan chiqish qopqoqlari va
  `provider_failure_threshold` deterministik ko'p tengdoshlarni olish imkonini beradi.
- **Profil identifikatorlari** — provayderlar kanonik tutqichni ochishi kerak (masalan,
  `sorafs.sf1@1.0.0`); ixtiyoriy `profile_aliases` eski mijozlarga migratsiyaga yordam beradi.

Tasdiqlash qoidalari nol stavkani, bo'sh qobiliyat/so'nggi nuqtalar/mavzu ro'yxatini rad etadi,
noto'g'ri ishlash muddati yoki etishmayotgan QoS maqsadlari. Qabul konvertlari taqqoslaydi
yangilanishlarni g'iybat qilishdan oldin reklama va taklif organlari (`compare_core_fields`).

### Diapazonni olish kengaytmalari

Diapazonga ega bo'lgan provayderlar quyidagi metama'lumotlarni o'z ichiga oladi:

| Maydon | Maqsad |
|-------|---------|
| `CapabilityType::ChunkRangeFetch` | `max_chunk_span`, `min_granularity` va tekislash/dalillash bayroqlarini e'lon qiladi. |
| `StreamBudgetV1` | Ixtiyoriy parallellik/oʻtkazish konverti (`max_in_flight`, `max_bytes_per_sec`, ixtiyoriy `burst`). Diapazon qobiliyatini talab qiladi. |
| `TransportHintV1` | Buyurtma qilingan transport afzalliklari (masalan, `torii_http_range`, `quic_stream`, `soranet_relay`). Ustuvorliklar `0–15` va dublikatlar rad etiladi. |

Asboblarni qo'llab-quvvatlash:

- Provayder reklama quvurlari diapazon qobiliyatini, oqim byudjetini va
  tekshiruvlar uchun deterministik foydali yuklarni chiqarishdan oldin transport maslahatlari.
- `cargo xtask sorafs-admission-fixtures` kanonik ko'p manbali to'plamlar
  ostidagi eskirish moslamalari bilan bir qatorda reklamalar
  `fixtures/sorafs_manifest/provider_admission/`.
- `stream_budget` yoki `transport_hints` ni o'tkazib yuboradigan diapazonli reklamalar
  ko'p manbani saqlab, rejalashtirishdan oldin CLI/SDK yuklagichlari tomonidan rad etilgan
  jabduqlar Torii qabul kutishlariga mos keladi.

## Gateway diapazonining so'nggi nuqtalari

Shlyuzlar reklama metama'lumotlarini aks ettiruvchi deterministik HTTP so'rovlarini qabul qiladi.

### `GET /v1/sorafs/storage/car/{manifest_id}`

| Talab | Tafsilotlar |
|-------------|---------|
| **Sarlavhalar** | `Range` (bo'lakli ofsetlarga moslangan yagona oyna), `dag-scope: block`, `X-SoraFS-Chunker`, ixtiyoriy `X-SoraFS-Nonce` va majburiy baza64 `X-SoraFS-Stream-Token`. |
| **Javoblar** | `206` `Content-Type: application/vnd.ipld.car`, xizmat koʻrsatilayotgan oynani tavsiflovchi `Content-Range`, `X-Sora-Chunk-Range` metamaʼlumotlari va aks-sadolangan chunker/token sarlavhalari. |
| **Muvaffaqiyatsizlik rejimlari** | Noto'g'ri moslashtirilgan diapazonlar uchun `416`, etishmayotgan/yaroqsiz tokenlar uchun `401`, oqim/bayt byudjetlari oshib ketganda `429`. |

### `GET /v1/sorafs/storage/chunk/{manifest_id}/{digest}`

Bir xil sarlavhalar va deterministik bo'laklar dayjesti bilan bitta bo'lakni olish.
CAR bo'laklari kerak bo'lmaganda qayta urinishlar yoki sud-tibbiy yuklamalar uchun foydalidir.

## Ko'p manbali orkestr ish jarayoni

SF-6 ko'p manbali olish yoqilganda (`sorafs_fetch` orqali Rust CLI,
`sorafs_orchestrator` orqali SDK'lar):

1. **Kirish maʼlumotlarini toʻplash** — manifest boʻlak rejasini dekodlash, eng soʻnggi reklamalarni olish,
   va ixtiyoriy ravishda telemetriya suratini uzating (`--telemetry-json` yoki
   `TelemetrySnapshot`).
2. **Tabel yarating** — `Orchestrator::build_scoreboard` baholaydi
   muvofiqlik va rad etish sabablarini qayd etish; `sorafs_fetch --scoreboard-out`
   JSONni davom ettiradi.
3. **Jadval qismlari** — `fetch_with_scoreboard` (yoki `--plan`) diapazonni amalga oshiradi
   cheklovlar, oqim byudjetlari, qayta urinish/teng chegaralari (`--retry-budget`,
   `--max-peers`) va har bir so'rov uchun manifest-ko'lamli oqim tokenini chiqaradi.
4. **Kvitansiyalarni tekshirish** — chiqishlar `chunk_receipts` va
   `provider_reports`; CLI xulosalari saqlanib qoladi `provider_reports`,
   Dalillar to'plami uchun `chunk_receipts` va `ineligible_providers`.

Operatorlar/SDKlar uchun keng tarqalgan xatolar:

| Xato | Tavsif |
|-------|-------------|
| `no providers were supplied` | Filtrdan so'ng tegishli yozuvlar yo'q. |
| `no compatible providers available for chunk {index}` | Muayyan bo'lak uchun diapazon yoki byudjet nomuvofiqligi. |
| `retry budget exhausted after {attempts}` | `--retry-budget` ni oshiring yoki muvaffaqiyatsiz tengdoshlarni chiqarib tashlang. |
| `no healthy providers remaining` | Takroriy nosozliklardan so'ng barcha provayderlar o'chirildi. |
| `streaming observer failed` | Downstream CAR yozuvchisi bekor qilindi. |
| `orchestrator invariant violated` | Triaj uchun manifest, skorbord, telemetriya surati va CLI JSONni suratga oling. |

## Telemetriya va dalillar

- Orkestr tomonidan chiqarilgan ko'rsatkichlar:  
  `sorafs_orchestrator_active_fetches`, `sorafs_orchestrator_fetch_duration_ms`,
  `sorafs_orchestrator_retries_total`, `sorafs_orchestrator_provider_failures_total`
  (manifest/mintaqa/provayder tomonidan belgilangan). `telemetry_region` ni konfiguratsiya yoki orqali o'rnating
  CLI bayroqlari, shuning uchun asboblar paneli parklar bo'yicha bo'linadi.
- CLI/SDK yig'ish xulosalari doimiy reyting jadvali JSON, bo'lak kvitansiyalari,
  va SF-6/SF-7 shlyuzlari uchun tarqatiladigan paketlarda jo'natilishi kerak bo'lgan provayder hisobotlari.
- Gateway ishlov beruvchilari `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error` ni ko'rsatadi
  shuning uchun SRE asboblar paneli orkestrator qarorlarini server xatti-harakatlari bilan bog'lashi mumkin.

## CLI va REST yordamchilari

- `iroha app sorafs pin list|show`, `alias list` va `replication list`
  pin-registr REST so'nggi nuqtalari va attestatsiya bloklari bilan xom Norito JSON chop etish
  auditorlik dalillari uchun.
- `iroha app sorafs storage pin` va `torii /v1/sorafs/pin/register` Noritoni qabul qiladi
  yoki JSON manifestlari va ixtiyoriy taxallus isbotlari va vorislari; noto'g'ri shakllangan dalillar
  `400` ko'taring, eskirgan sirtni `503` bilan `Warning: 110` va
  muddati o'tgan dalillar `412` ni qaytaradi.
- `iroha app sorafs repair list` nometall navbat filtrlarini ta'mirlash, esa
  `repair claim|complete|fail|escalate` imzolangan ishchi harakatlari yoki slash
  Torii ga takliflar. Slash takliflari boshqaruvni tasdiqlash xulosasini o'z ichiga olishi mumkin
  (Ovozlarni hisoblashni tasdiqlash/rad etish/betaraf qilish va tasdiqlangan_at/finalized_at
  vaqt belgilari); mavjud bo'lganda u kvorum va nizolar/apellyatsiya shartlarini qondirishi kerak,
  aks holda, taklif belgilangan muddatda ovozlar hal etilgunga qadar bahsli bo'lib qoladi.
- Ta'mirlash ro'yxati va ishchilar navbatini tanlash SLA muddati, nosozlikning jiddiyligi va deterministik bog'lovchilar (navbat vaqti, manifest dayjesti, chipta identifikatori) bilan provayderning kechikishi bo'yicha tartibga solinadi.
- Ta'mirlash holatiga javoblar orasida base64 Norito ni o'z ichiga olgan `events` qatori mavjud
  `RepairTaskEventV1` yozuvlari audit izlari uchun sodir bo'lishiga qarab tartiblangan; ro'yxat
  eng so'nggi o'tishlar bilan cheklangan.
- `iroha app sorafs gc inspect|dry-run --data-dir=/var/lib/sorafs` faqat o'qish uchun chiqaradi
  audit dalillari uchun mahalliy manifest do'konidan saqlash hisobotlari.
- REST oxirgi nuqtalari (`/v1/sorafs/pin`, `/v1/sorafs/aliases`,
  `/v1/sorafs/replication`) mijozlar uchun attestatsiya tuzilmalarini o'z ichiga oladi
  chora ko'rishdan oldin ma'lumotlarni so'nggi blok sarlavhalari bilan tekshiring.

## Ma'lumotnomalar

- Kanonik spetsifikatsiya:
  [`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
- Norito turlari: `crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- CLI yordamchilari: `crates/iroha_cli/src/commands/sorafs.rs`,
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- Orkestr qutisi: `crates/sorafs_orchestrator`
- Boshqaruv paneli to'plami: `dashboards/grafana/sorafs_fetch_observability.json`