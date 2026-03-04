---
lang: uz
direction: ltr
source: docs/source/da/replication_policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 70163ed6740c80c48c78ae918c37d34e0022ab97ffabce6d451bbf85060e24b4
source_last_modified: "2026-01-22T14:35:37.691616+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Ma'lumotlar mavjudligini takrorlash siyosati (DA-4)

_Holat: Davom etmoqda — Egalari: Asosiy Protokol WG / Saqlash jamoasi / SRE_

DA qabul qilish quvuri endi deterministik saqlash maqsadlarini amalga oshiradi
`roadmap.md` da tasvirlangan har bir blob sinfi (DA-4 ishchi oqimi). Torii rad etadi
sozlanganlarga mos kelmaydigan qo'ng'iroq qiluvchi tomonidan taqdim etilgan saqlash konvertlarini saqlab qolish
siyosat, har bir validator/saqlash tugunlari talab qilinadiganni saqlab qolishini kafolatlaydi
topshiruvchi niyatiga tayanmasdan davrlar va nusxalar soni.

## Standart siyosat

| Blob sinf | Issiq saqlash | Sovuqni ushlab turish | Kerakli nusxalar | Saqlash sinfi | Boshqaruv tegi |
|------------|---------------|----------------|-------------------|----------------|----------------|
| `taikai_segment` | 24 soat | 14 kun | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 soat | 7 kun | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 soat | 180 kun | 3 | `cold` | `da.governance` |
| _Default (barcha boshqa sinflar)_ | 6 soat | 30 kun | 3 | `warm` | `da.default` |

Bu qiymatlar `torii.da_ingest.replication_policy` ichiga kiritilgan va ularga qo'llaniladi
barcha `/v1/da/ingest` taqdimotlari. Torii manifestlarni majburiy bilan qayta yozadi
saqlash profili va qo'ng'iroq qiluvchilar mos kelmaydigan qiymatlarni taqdim etganda ogohlantirish chiqaradi
operatorlar eskirgan SDK larni aniqlay oladi.

### Taikai mavjudligi darslari

Taikai marshrutlash manifestlari (`taikai.trm` metadata) endi
`availability_class` maslahati (`Hot`, `Warm` yoki `Cold`). Mavjud bo'lganda, Torii
`torii.da_ingest.replication_policy` dan mos saqlash profilini tanlaydi
foydali yukni bo'laklashdan oldin, voqea operatorlariga nofaollik darajasini pasaytirish imkonini beradi
global siyosat jadvalini tahrir qilmasdan tarjima qilish. Standartlar quyidagilardir:

| Mavjudlik klassi | Issiq saqlash | Sovuqni ushlab turish | Kerakli nusxalar | Saqlash sinfi | Boshqaruv tegi |
|--------------------|---------------|----------------|-------------------|----------------|----------------|
| `hot` | 24 soat | 14 kun | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 soat | 30 kun | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 soat | 180 kun | 3 | `cold` | `da.taikai.archive` |

Agar manifestda `availability_class` qoldirilsa, qabul qilish yo'li yana qaytib keladi.
`hot` profili, shuning uchun jonli oqimlar o'zlarining to'liq replika to'plamini saqlab qoladilar. Operatorlar mumkin
yangisini tahrirlash orqali ushbu qiymatlarni bekor qiling
Konfiguratsiyadagi `torii.da_ingest.replication_policy.taikai_availability` bloki.

## Konfiguratsiya

Siyosat `torii.da_ingest.replication_policy` ostida amal qiladi va a
*standart* shablon va har bir sinf uchun bekor qilishlar qatori. Sinf identifikatorlari
katta-kichik harflarni sezmaydi va `taikai_segment`, `nexus_lane_sidecar`,
`governance_artifact` yoki boshqaruv tomonidan tasdiqlangan kengaytmalar uchun `custom:<u16>`.
Saqlash sinflari `hot`, `warm` yoki `cold` ni qabul qiladi.

```toml
[torii.da_ingest.replication_policy.default_retention]
hot_retention_secs = 21600          # 6 h
cold_retention_secs = 2592000       # 30 d
required_replicas = 3
storage_class = "warm"
governance_tag = "da.default"

[[torii.da_ingest.replication_policy.overrides]]
class = "taikai_segment"
[torii.da_ingest.replication_policy.overrides.retention]
hot_retention_secs = 86400          # 24 h
cold_retention_secs = 1209600       # 14 d
required_replicas = 5
storage_class = "hot"
governance_tag = "da.taikai.live"
```

Yuqorida sanab o'tilgan standart sozlamalar bilan ishlash uchun blokni tegmasdan qoldiring. Tortish uchun a
sinf, mos keladigan bekor qilishni yangilash; yangi sinflar uchun bazani o'zgartirish,
tahrirlash `default_retention`.Muayyan Taikai mavjudlik sinflarini sozlash uchun ostidagi yozuvlarni qo'shing
`torii.da_ingest.replication_policy.taikai_availability`:

```toml
[[torii.da_ingest.replication_policy.taikai_availability]]
availability_class = "warm"
[torii.da_ingest.replication_policy.taikai_availability.retention]
hot_retention_secs = 14400         # 4 h
cold_retention_secs = 604800       # 7 d
required_replicas = 4
storage_class = "warm"
governance_tag = "da.taikai.warm"
```

## Amalga oshirish semantikasi

- Torii foydalanuvchi tomonidan taqdim etilgan `RetentionPolicy` ni majburiy profil bilan almashtiradi
  parchalanish yoki aniq emissiyadan oldin.
- Noto'g'ri saqlash profilini e'lon qiluvchi oldindan tuzilgan manifestlar rad etiladi
  `400 schema mismatch` bilan, shuning uchun eskirgan mijozlar shartnomani zaiflashtira olmaydi.
- Har bir bekor qilish hodisasi qayd qilinadi (`blob_class`, taqdim etilgan va kutilgan siyosat)
  tarqatish paytida mos kelmaydigan qo'ng'iroq qiluvchilarni yuzaga chiqarish.

Yangilangan eshik uchun `docs/source/da/ingest_plan.md` (Tasdiqlash roʻyxati) ga qarang.
saqlash majburiyatlarini qamrab oladi.

## Qayta nusxalash ish jarayoni (DA-4 kuzatuvi)

Saqlash majburiyati faqat birinchi qadamdir. Operatorlar ham buni isbotlashlari kerak
jonli manifestlar va replikatsiya buyurtmalari sozlangan siyosatga mos keladi
SoraFS mos kelmaydigan bloblarni avtomatik ravishda takrorlashi mumkin.

1. **Driftni kuzating.** Torii chiqaradi
   `overriding DA retention policy to match configured network baseline` har doim
   qo'ng'iroq qiluvchi eski saqlash qiymatlarini yuboradi. Bu jurnal bilan ulang
   `torii_sorafs_replication_*` replika etishmovchiligi yoki kechikishini aniqlash uchun telemetriya
   qayta joylashtirishlar.
2. **Maqsad va jonli nusxalar farqi.** Yangi audit yordamchisidan foydalaning:

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   Buyruq taqdim etilganlardan `torii.da_ingest.replication_policy` ni yuklaydi
   config, har bir manifestni dekodlaydi (JSON yoki Norito) va ixtiyoriy ravishda istalganiga mos keladi.
   Manifest dayjest bo'yicha `ReplicationOrderV1` foydali yuklar. Xulosa ikkita bayroqcha
   shartlar:

   - `policy_mismatch` - manifestni saqlash profili majburiy holatdan farq qiladi
     siyosat (Torii noto'g'ri sozlanmagan bo'lsa, bu hech qachon sodir bo'lmasligi kerak).
   - `replica_shortfall` - jonli replikatsiya buyurtmasi kamroq replikalarni talab qiladi
     `RetentionPolicy.required_replicas` yoki unga qaraganda kamroq topshiriqlar beradi
     maqsad.

   Nolga teng bo'lmagan chiqish holati faol tanqislikni bildiradi, shuning uchun CI/chaqiruvda avtomatlashtirish
   darhol sahifani ochishingiz mumkin. JSON hisobotini ilovaga biriktiring
   Parlament ovozlari uchun `docs/examples/da_manifest_review_template.md` paketi.
3. **Replikatsiyani ishga tushiring.** Audit etishmovchilik haqida xabar berganda, yangisini chiqaring.
   `ReplicationOrderV1` da tavsiflangan boshqaruv vositalari orqali
   `docs/source/sorafs/storage_capacity_marketplace.md` va auditni qayta ishga tushiring
   replika to'plami birlashmaguncha. Favqulodda vaziyatni bekor qilish uchun CLI chiqishini ulang
   `iroha app da prove-availability` bilan SRElar bir xil dayjestga murojaat qilishlari mumkin
   va PDP dalillari.

Regressiya qamrovi `integration_tests/tests/da/replication_policy.rs` da yashaydi;
to'plam `/v1/da/ingest` ga mos kelmaydigan saqlash siyosatini taqdim etadi va tekshiradi
olingan manifest qo'ng'iroq qiluvchining o'rniga majburiy profilni ochib beradi
niyat.

## Sog'liqni saqlashni tasdiqlovchi telemetriya va asboblar paneli (DA-5 ko'prigi)

“Yoʻl xaritasi” bandi **DA-5** PDP/PoTR ijrosi natijalarini tekshirish mumkin boʻlishini talab qiladi.
real vaqt. `SorafsProofHealthAlert` voqealari endi maxsus to'plamni boshqaradi
Prometheus ko'rsatkichlari:

- `torii_sorafs_proof_health_alerts_total{provider_id,trigger,penalty}`
- `torii_sorafs_proof_health_pdp_failures{provider_id}`
- `torii_sorafs_proof_health_potr_breaches{provider_id}`
- `torii_sorafs_proof_health_penalty_nano{provider_id}`
- `torii_sorafs_proof_health_cooldown{provider_id}`
- `torii_sorafs_proof_health_window_end_epoch{provider_id}`

**SoraFS PDP & PoTR Health** Grafana taxtasi
(`dashboards/grafana/sorafs_pdp_potr_health.json`) endi bu signallarni ochib beradi:- *Trigger tomonidan tasdiqlangan sog'liq ogohlantirishlari* ogohlantirish stavkalarini trigger/jarima bayrog'i bo'yicha diagrammada ko'rsatadi.
  Taikai/CDN operatorlari faqat PDP, faqat PoTR yoki ikkilamchi ogohlantirishlar mavjudligini isbotlashlari mumkin.
  otish.
- *Provayderlar Cooldown* hozirda a ostida boʻlgan provayderlarning jonli summasi haqida xabar beradi
  SorafsProofHealthAlert sovutish vaqti.
- *Proof Health Window Snapshot* PDP/PoTR hisoblagichlarini, jarima miqdorini birlashtiradi,
  Sovutish bayrog'i va provayder boshiga ogohlantirish oynasi tugashi davri, shuning uchun boshqaruv sharhlovchilari
  jadvalni hodisa paketlariga biriktirishi mumkin.

Runbooks DA ijro dalillarini taqdim etishda ushbu panellarni bog'lashi kerak; ular
CLI proof-stream muvaffaqiyatsizliklarini to'g'ridan-to'g'ri zanjirdagi jarima metama'lumotlariga bog'lang va
yo'l xaritasida ko'rsatilgan kuzatuv kancasini taqdim eting.