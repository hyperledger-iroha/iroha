---
id: node-storage
lang: uz
direction: ltr
source: docs/portal/docs/sorafs/node-storage.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Node Storage Design
sidebar_label: Node Storage Design
description: Storage architecture, quotas, and lifecycle hooks for Torii nodes hosting SoraFS data.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: Eslatma Kanonik manba
:::

## SoraFS tugunni saqlash dizayni (qoralama)

Bu eslatma Iroha (Torii) tugunining SoraFS maʼlumotlariga qanday ulanishi mumkinligini aniqlaydi.
mavjudlik qatlami va saqlash va xizmat ko'rsatish uchun mahalliy diskning bir bo'lagini ajrating
bo'laklar. U `sorafs_node_client_protocol.md` kashfiyot spetsifikatsiyasini to'ldiradi va
SF-1b armatura saqlash tomoni arxitekturasini, resursini belgilash orqali ishlaydi
boshqaruv elementlari va tugun va shlyuzga tushishi kerak bo'lgan konfiguratsiya sanitariya-tesisat
kod yo'llari. Amaliy operator matkaplari yashaydi
[Tugun operatsiyalari Runbook](./node-operations).

### Maqsadlar

- Har qanday validator yoki yordamchi Iroha jarayoniga zaxira disk sifatida foydalanishga ruxsat bering.
  SoraFS provayderi daftarning asosiy majburiyatlariga ta'sir qilmasdan.
- Saqlash modulini deterministik va Norito boshqaruvida saqlang: manifestlar,
  chunk rejalari, Proof-of-Retrievability (PoR) ildizlari va provayder reklamalari
  haqiqat manbai.
- Tugun o'z resurslarini sarflamasligi uchun operator tomonidan belgilangan kvotalarni qo'llash
  juda ko'p pin yoki olish so'rovlarini qabul qilish.
- Yuzaki sog'liq/telemetriya (PoR namunasi, bo'laklarni olish kechikishi, disk bosimi)
  boshqaruv va mijozlarga qaytish.

### Yuqori darajadagi arxitektura

```
┌──────────────────────────────────────────────────────────────────────┐
│                         Iroha/Torii Node                             │
│                                                                      │
│  ┌──────────────┐      ┌────────────────────┐                        │
│  │  Torii APIs  │◀────▶│   SoraFS Gateway   │◀───────────────┐       │
│  └──────────────┘      │ (Norito endpoints) │                │       │
│                        └────────┬───────────┘                │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Pin Registry   │◀───── manifests   │       │
│                        │ (State / DB)    │                   │       │
│                        └────────┬────────┘                   │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Chunk Storage  │◀──── chunk plans  │       │
│                        │  (ChunkStore)   │                   │       │
│                        └────────┬────────┘                   │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Disk Quota/IO  │─Pin/serve chunks─▶│ Fetch │
│                        │  Scheduler      │                   │ Clients│
│                        └─────────────────┘                   │       │
│                                                                      │
└──────────────────────────────────────────────────────────────────────┘
```

Asosiy modullar:

- **Gateway**: pin takliflari, parchalarni olish uchun Norito HTTP so'nggi nuqtalarini ochib beradi
  so'rovlar, PoR namunalarini olish va telemetriya. U Norito foydali yuklarni tasdiqlaydi va
  marshallar parchalar do'koniga so'rovlar yuboradi. Mavjud Torii HTTP stekini qayta ishlatadi
  yangi demondan qochish uchun.
- **Pin registri**: `iroha_data_model::sorafs` da kuzatilgan manifest pin holati
  va `iroha_core`. Manifest qabul qilinganda, ro'yxatga olish kitobi qayd qiladi
  manifest dayjesti, chunk plan dayjesti, PoR ildizi va provayder qobiliyati bayroqlari.
- **Chunk Storage**: disk tomonidan qo'llab-quvvatlanadigan `ChunkStore` ilovasi qabul qilinadi
  imzolangan manifestlar, `ChunkProfile::DEFAULT` yordamida parcha rejalarini amalga oshiradi va
  deterministik tartib ostida bo'laklarni saqlaydi. Har bir bo'lak a bilan bog'langan
  kontent barmoq izi va PoR metadata, shuning uchun namuna olishsiz qayta tekshirish mumkin
  butun faylni qayta o'qish.
- **Kvota/Scheduler**: operator tomonidan sozlangan chegaralarni (maksimal disk baytlari,
  maksimal ajoyib pinlar, maksimal parallel olishlar, chunk TTL) va koordinatalar
  IO, shuning uchun tugunning hisob kitobi vazifalari och qolmaydi. Rejalashtiruvchi ham
  Cheklangan CPU bilan PoR dalillari va namuna olish so'rovlariga xizmat ko'rsatish uchun javobgar.

### Konfiguratsiya

`iroha_config` ga yangi bo'lim qo'shing:

```toml
[sorafs.storage]
enabled = false
data_dir = "/var/lib/iroha/sorafs"
max_capacity_bytes = "100 GiB"
max_parallel_fetches = 32
max_pins = 10_000
por_sample_interval_secs = 600
alias = "tenant.alpha"            # optional human friendly tag
adverts:
  stake_pointer = "stake.pool.v1:0x1234"
  availability = "hot"
  max_latency_ms = 500
  topics = ["sorafs.sf1.primary:global"]
```

- `enabled`: ishtirok etish tugmasi. False bo'lsa, shlyuz 503 for ni qaytaradi
  saqlash so'nggi nuqtalari va tugun kashfiyotda reklama qilmaydi.
- `data_dir`: parcha ma'lumotlari, PoR daraxtlari va telemetriyani olish uchun ildiz katalogi.
  Standart `<iroha.data_dir>/sorafs`.
- `max_capacity_bytes`: mahkamlangan bo'lak ma'lumotlari uchun qattiq chegara. Fon vazifasi
  chegaraga yetganda yangi pinlarni rad etadi.
- `max_parallel_fetches`: muvozanat uchun rejalashtiruvchi tomonidan qo'llaniladigan parallellik chegarasi
  validator ish yukiga qarshi tarmoqli kengligi/disk IO.
- `max_pins`: qo'llashdan oldin tugun qabul qiladigan manifest pinlarining maksimal soni
  evakuatsiya / orqaga bosim.
- `por_sample_interval_secs`: avtomatik PoR namuna olish ishlari uchun kadans. Har bir ish
  `N` namunalari (har bir manifestda sozlanishi) tark etadi va telemetriya hodisalarini chiqaradi.
  Boshqaruv `N` sig‘im metama’lumotlarini o‘rnatish orqali qat’iy ravishda masshtablashi mumkin
  kalit `profile.sample_multiplier` (butun son `1-4`). Qiymat bitta bo'lishi mumkin
  raqam/string yoki har bir profilni bekor qiluvchi ob'ekt, masalan.
  `{"default":2,"sorafs.sf2@1.0.0":3}`.
- `adverts`: to'ldirish uchun provayder reklama generatori tomonidan ishlatiladigan tuzilma
  `ProviderAdvertV1` maydonlari (qoida ko'rsatkichi, QoS bo'yicha maslahatlar, mavzular). Agar o'tkazib yuborilgan bo'lsa
  tugun boshqaruv registridagi standart sozlamalardan foydalanadi.

Santexnika konfiguratsiyasi:

- `[sorafs.storage]` `iroha_config` da `SorafsStorage` sifatida belgilangan va
  tugun konfiguratsiya faylidan yuklangan.
- `iroha_core` va `iroha_torii` saqlash konfiguratsiyasini shlyuzga ulang
  ishga tushirilganda quruvchi va chunk do'koni.
- Dev/test env bekor qilishlari mavjud (`SORAFS_STORAGE_*`, `SORAFS_STORAGE_PIN_*`), lekin
  ishlab chiqarishni joylashtirish konfiguratsiya fayliga tayanishi kerak.

### CLI Utilities

Torii ning HTTP yuzasi hali ham simli bo'lsa-da, `sorafs_node` qutisi
yupqa CLI, shuning uchun operatorlar doimiy ma'lumotlarga qarshi matkaplarni qabul qilish/eksport qilishlari mumkin.
backend.【crates/sorafs_node/src/bin/sorafs-node.rs:1】

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin \
  --plan-json-out ./plan.json
```

- `ingest` Norito kodli manifest `.to` faylini va unga mos keladigan yukni kutadi
  bayt. U manifestning bo'linish profilidan bo'lak rejasini qayta tiklaydi,
  digest paritetini ta'minlaydi, parcha fayllarni davom ettiradi va ixtiyoriy ravishda a chiqaradi
  `chunk_fetch_specs` JSON blob, shuning uchun quyi oqim asboblari aql-idrokni tekshirishi mumkin
  tartib.
- `export` manifest identifikatorini qabul qiladi va saqlangan manifest/foydali yukni diskka yozadi
  (ixtiyoriy JSON rejasi bilan), shuning uchun armatura muhitlar bo'ylab takrorlanishi mumkin.

Ikkala buyruq ham Norito JSON xulosasini stdout-ga bosib chiqaradi, bu esa uni o'tkazishni osonlashtiradi.
skriptlar. CLI manifestlarni ta'minlash uchun integratsiya testi bilan qoplangan
foydali yuklar Torii API'lari qo'nmasdan oldin bo'ylab toza.【crates/sorafs_node/tests/cli.rs:1】

> HTTP pariteti
>
> Torii shlyuzi endi faqat oʻqish uchun moʻljallangan yordamchilarni ochib beradi.
> `NodeHandle`:
>
> - `GET /v2/sorafs/storage/manifest/{manifest_id_hex}` — saqlanganni qaytaradi
> Norito manifest (base64) digest/metadata bilan birga.【crates/iroha_torii/src/sorafs/api.rs:1207】
> - `GET /v2/sorafs/storage/plan/{manifest_id_hex}` — deterministikni qaytaradi
> quyi oqim asboblari uchun JSON (`chunk_fetch_specs`) bo'lak rejasi.【crates/iroha_torii/src/sorafs/api.rs:1259】
>
> Ushbu so'nggi nuqtalar CLI chiqishini aks ettiradi, shuning uchun quvurlar mahalliydan o'tishlari mumkin
> skriptlarni tahlil qiluvchilarni oʻzgartirmasdan HTTP problariga.【crates/iroha_torii/src/sorafs/api.rs:1207】【crates/iroha_torii/src/sorafs/api.rs:1259】

### Tugunning hayot aylanishi

1. **Ishga tushirish**:
   - Agar saqlash yoqilgan bo'lsa, tugun parchalar do'konini ishga tushiradi
     sozlangan katalog va imkoniyatlar. Bunga tekshirish yoki yaratish kiradi
     PoR manifest ma'lumotlar bazasi va biriktirilgan manifestlarni issiq keshlarga qayta o'ynash.
   - SoraFS shlyuz marshrutlarini ro'yxatdan o'tkazing (pin uchun Norito JSON POST/GET so'nggi nuqtalari,
     olish, PoR namunasi, telemetriya).
   - PoR namuna olish ishchisi va kvota monitorini yarating.
2. **Kashfiyot / Reklamalar**:
   - Joriy imkoniyatlar/salomatlikdan foydalangan holda `ProviderAdvertV1` hujjatlarini yarating, belgilang
     ularni kengash tomonidan tasdiqlangan kalit bilan va kashfiyot kanali orqali nashr eting.
     mavjud.
3. **Pin ish jarayoni**:
   - Gateway imzolangan manifestni oladi (jumladan, bo'lak rejasi, PoR ildizi, kengash
     imzolar). Taxallus ro'yxatini tasdiqlang (`sorafs.sf1@1.0.0` kerak) va
     chunk rejasi manifest metama'lumotlariga mos kelishiga ishonch hosil qiling.
   - Kvotalarni tekshiring. Imkoniyatlar/pin chegaralari oshib ketgan bo'lsa, a bilan javob bering
     siyosat xatosi (Norito tuzilgan).
   - `ChunkStore`-ga parcha ma'lumotlarini uzating, biz qabul qilganimizda hazm qilishni tekshiramiz.
     PoR daraxtlarini yangilang va manifest metama'lumotlarini ro'yxatga olish kitobida saqlang.
4. **Ish jarayonini olish**:
   - Diskdan bo'lak diapazoni so'rovlariga xizmat ko'rsatish. Rejalashtiruvchi amal qiladi
     `max_parallel_fetches` va to'yingan bo'lsa `429` qaytaradi.
   - Strukturaviy telemetriyani (Norito JSON) kechikish, xizmat koʻrsatish baytlari va
     quyi oqim monitoringi uchun xatoliklarni hisoblash.
5. **PoR namunasi**:
   - Ishchi og'irligiga mutanosib manifestlarni tanlaydi (masalan, saqlangan baytlar) va
     chunk do'konining PoR daraxti yordamida deterministik namuna olishni amalga oshiradi.
   - Boshqaruv tekshiruvlari natijalarini davom ettirish va provayderga xulosalarni kiritish
     reklamalar / telemetriya so'nggi nuqtalari.
6. **Ko‘chirish/kvota ijrosi**:
   - Imkoniyatga erishilganda tugun sukut bo'yicha yangi pinlarni rad etadi. Majburiy emas,
     operatorlar ko'chirish siyosatini (masalan, TTL-ga asoslangan, LRU) bir marta sozlashi mumkin
     boshqaruv modeli kelishilgan; hozircha dizayn qat'iy kvotalarni o'z zimmasiga oladi va
     operator tashabbusi bilan olib tashlash operatsiyalari.

### Imkoniyatlar deklaratsiyasi va rejalashtirish integratsiyasi- Torii endi `/v2/sorafs/capacity/declare` dan `CapacityDeclarationRecord` yangilanishlarini uzatadi
  o'rnatilgan `CapacityManager` ga, shuning uchun har bir tugun o'zining xotiradagi ko'rinishini yaratadi
  chunker va yo'laklarni ajratishni amalga oshirdi. Menejer faqat o'qish uchun mo'ljallangan suratlarni ko'rsatadi
  telemetriya uchun (`GET /v2/sorafs/capacity/state`) va har bir profil yoki har bir qator uchun amal qiladi
  yangi buyurtmalar qabul qilinishidan oldin bandlovlar.【crates/sorafs_node/src/capacity.rs:1】【crates/sorafs_node/src/lib.rs:60】
- `/v2/sorafs/capacity/schedule` oxirgi nuqtasi boshqaruv tomonidan chiqarilgan `ReplicationOrderV1` ni qabul qiladi
  foydali yuklar. Buyurtma mahalliy provayderga mo'ljallangan bo'lsa, menejer tekshiradi
  takroriy rejalashtirish, chunker/yo'l sig'imini tekshiradi, bo'lakni zahiraga oladi va
  `ReplicationPlan` ni qaytaradi, bu esa qolgan quvvatni tavsiflaydi, shuning uchun orkestrlash vositalari
  yutish bilan davom etishi mumkin. Boshqa provayderlar uchun buyurtmalar a bilan tasdiqlanadi
  Ko'p operatorli ish oqimlarini osonlashtirish uchun `ignored` javobi.【crates/iroha_torii/src/routing.rs:4845】
- Tugatish ilgaklari (masalan, qabul qilish muvaffaqiyatli bo'lganidan keyin ishga tushiriladi) uriladi
  `POST /v2/sorafs/capacity/complete` orqali bandlovlarni chiqarish
  `CapacityManager::complete_order`. Javobda `ReplicationRelease` mavjud
  oniy tasvir (qolgan jami, chunker/bo'lak qoldiqlari), shuning uchun orkestrlash vositalari
  keyingi buyurtmani ovoz berishsiz navbatga qo'ying. Keyingi ishlar buni bo'lakka o'tkazadi
  do'kon quvur liniyasi bir marta yutish mantiq erlari.【crates/iroha_torii/src/routing.rs:4885】【crates/sorafs_node/src/capacity.rs:90】
- O'rnatilgan `TelemetryAccumulator` orqali mutatsiyaga uchragan bo'lishi mumkin
  `NodeHandle::update_telemetry`, fon ishchilariga PoR/ish vaqti namunalarini yozib olish imkonini beradi
  va oxir-oqibat, `CapacityTelemetryV1` kanonik foydali yuklarini teginmasdan oling.
  rejalashtiruvchining ichki qismlari.【crates/sorafs_node/src/lib.rs:142】【crates/sorafs_node/src/telemetry.rs:1】

### Integratsiya va kelajak ishlari

- **Boshqaruv**: saqlash telemetriyasi bilan `sorafs_pin_registry_tracker.md`ni kengaytiring
  (PoR muvaffaqiyat darajasi, diskdan foydalanish). Qabul qilish qoidalari minimal talab qilishi mumkin
  sig'im yoki reklama qabul qilinishidan oldin minimal PoR muvaffaqiyat darajasi.
- **Client SDKs**: yangi xotira konfiguratsiyasini (disk chegaralari, taxallus) oching
  boshqaruv vositalari tugunlarni dasturiy ravishda yuklashi mumkin.
- **Telemetriya**: mavjud ko'rsatkichlar to'plami bilan integratsiyalash (Prometheus /
  OpenTelemetry) shuning uchun saqlash ko'rsatkichlari kuzatuv panelida ko'rinadi.
- **Xavfsizlik**: saqlash modulini maxsus asenkron vazifa hovuzida ishga tushiring
  orqaga bosim o'tkazing va io_uring yoki tokio's orqali sandboxing qismini o'qishni ko'rib chiqing
  zararli mijozlarning resurslarni sarflashini oldini olish uchun cheklangan hovuzlar.

Ushbu dizayn saqlash modulini berishda ixtiyoriy va deterministik saqlaydi
operatorlar SoraFS ma'lumotlarining mavjudligida ishtirok etishlari kerak bo'lgan tugmalar
qatlam. Uni amalga oshirish `iroha_config`, `iroha_core`,
`iroha_torii` va Norito shlyuzi, shuningdek, provayder reklama vositalari.