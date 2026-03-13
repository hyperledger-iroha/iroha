---
lang: uz
direction: ltr
source: docs/portal/docs/da/replication-policy.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

sarlavha: Ma'lumotlar mavjudligini ko'paytirish siyosati
sidebar_label: Replikatsiya siyosati
tavsif: Boshqaruv tomonidan talab qilinadigan saqlash profillari barcha DA ingest taqdimnomalariga qo‘llaniladi.
---

::: Eslatma Kanonik manba
:::

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
barcha `/v2/da/ingest` taqdimotlari. Torii manifestlarni majburiy bilan qayta yozadi
saqlash profili va qo'ng'iroq qiluvchilar mos kelmaydigan qiymatlarni taqdim etganda ogohlantirish chiqaradi
operatorlar eskirgan SDK larni aniqlay oladi.

### Taikai mavjudligi darslari

Taikai marshrutlash manifestlari (`taikai.trm`) `availability_class` ni e'lon qiladi
(`hot`, `warm` yoki `cold`). Torii bo'linishdan oldin mos keladigan siyosatni amalga oshiradi
shuning uchun operatorlar har bir oqim uchun replikatsiyalar sonini globalni tahrir qilmasdan o'lchashlari mumkin
stol. Standart sozlamalar:

| Mavjudlik klassi | Issiq saqlash | Sovuqni ushlab turish | Kerakli nusxalar | Saqlash sinfi | Boshqaruv tegi |
|--------------------|---------------|----------------|-------------------|----------------|----------------|
| `hot` | 24 soat | 14 kun | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 soat | 30 kun | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 soat | 180 kun | 3 | `cold` | `da.taikai.archive` |

Yo‘qolgan maslahatlar standarti `hot`, shuning uchun jonli translyatsiyalar eng kuchli siyosatni saqlab qoladi.
orqali standart sozlamalarni bekor qiling
Tarmoqingiz foydalansa, `torii.da_ingest.replication_policy.taikai_availability`
turli maqsadlar.

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
tahrirlash `default_retention`.

Taikai mavjudligi sinflari orqali mustaqil ravishda bekor qilinishi mumkin
`torii.da_ingest.replication_policy.taikai_availability`:

```toml
[[torii.da_ingest.replication_policy.taikai_availability]]
availability_class = "cold"
[torii.da_ingest.replication_policy.taikai_availability.retention]
hot_retention_secs = 3600          # 1 h
cold_retention_secs = 15552000     # 180 d
required_replicas = 3
storage_class = "cold"
governance_tag = "da.taikai.archive"
```

## Amalga oshirish semantikasi

- Torii foydalanuvchi tomonidan taqdim etilgan `RetentionPolicy` ni majburiy profil bilan almashtiradi
  parchalanish yoki aniq emissiyadan oldin.
- Noto'g'ri saqlash profilini e'lon qiluvchi oldindan tuzilgan manifestlar rad etiladi
  `400 schema mismatch` bilan, shuning uchun eskirgan mijozlar shartnomani zaiflashtira olmaydi.
- Har bir bekor qilish hodisasi qayd qilinadi (`blob_class`, taqdim etilgan va kutilgan siyosat)
  tarqatish paytida mos kelmaydigan qo'ng'iroq qiluvchilarni yuzaga chiqarish.

Yangilangan darvoza uchun [Maʼlumotlar mavjudligini qabul qilish rejasi](ingest-plan.md) (Tasdiqlash roʻyxati) ga qarang.
saqlash majburiyatlarini qamrab oladi.

## Qayta nusxalash ish jarayoni (DA-4 kuzatuvi)

Saqlash majburiyati faqat birinchi qadamdir. Operatorlar ham buni isbotlashlari kerak
jonli manifestlar va replikatsiya buyurtmalari sozlangan siyosatga mos keladi
SoraFS mos kelmaydigan bloblarni avtomatik ravishda takrorlashi mumkin.

1. **Driftni kuzating.** Torii chiqaradi
   `overriding DA retention policy to match configured network baseline` har doim
   qo'ng'iroq qiluvchi eski saqlash qiymatlarini yuboradi. Bu jurnal bilan ulang
   `torii_sorafs_replication_*` replika kamchiliklarini yoki kechikishlarni aniqlash uchun telemetriya
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
   `docs/examples/da_manifest_review_template.md`
   Parlament ovozlari uchun paket.
3. **Replikatsiyani ishga tushiring.** Audit etishmovchilik haqida xabar berganda, yangisini chiqaring.
   `ReplicationOrderV1` da tavsiflangan boshqaruv vositalari orqali
   [SoraFS saqlash hajmi bozori](../sorafs/storage-capacity-marketplace.md) va auditni qaytadan o'tkazing
   replika to'plami birlashmaguncha. Favqulodda vaziyatni bekor qilish uchun CLI chiqishini ulang
   `iroha app da prove-availability` bilan SRElar bir xil dayjestga murojaat qilishlari mumkin
   va PDP dalillari.

Regressiya qamrovi `integration_tests/tests/da/replication_policy.rs` da yashaydi;
to'plam `/v2/da/ingest` ga mos kelmaydigan saqlash siyosatini taqdim etadi va tekshiradi
olingan manifest qo'ng'iroq qiluvchining o'rniga majburiy profilni ochib beradi
niyat.