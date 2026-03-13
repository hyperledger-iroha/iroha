---
lang: uz
direction: ltr
source: docs/portal/docs/sorafs/storage-capacity-marketplace.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 868edd6aa7401c64b8757db188edb13aa8e6ca8959966b6fea02e44bc298c6b7
source_last_modified: "2026-01-05T09:28:11.910794+00:00"
translation_last_reviewed: 2026-02-07
id: storage-capacity-marketplace
title: SoraFS Storage Capacity Marketplace
sidebar_label: Capacity Marketplace
description: SF-2c plan for the capacity marketplace, replication orders, telemetry, and governance hooks.
translator: machine-google-reviewed
---

::: Eslatma Kanonik manba
:::

# SoraFS Saqlash hajmi bozori (SF-2c qoralama)

SF-2c yo'l xaritasi punkti saqlanadigan boshqariladigan bozorni taqdim etadi
provayderlar o'z imkoniyatlarini e'lon qiladi, replikatsiya buyurtmalarini oladi va to'lovlarni oladi
yetkazib berish mavjudligiga mutanosib. Ushbu hujjat topshiriqlarni qamrab oladi
birinchi reliz uchun talab qilinadi va ularni harakatga keltiradigan treklarga ajratadi.

## Maqsadlar

- Ekspress provayder sig'imi majburiyatlari (jami baytlar, har bir qator chegaralari, amal qilish muddati)
  boshqaruv, SoraNet transport va Torii tomonidan iste'mol qilinadigan tekshiriladigan shaklda.
- E'lon qilingan imkoniyatlarga, ulushga va provayderlar bo'ylab pinlarni taqsimlang
  deterministik xatti-harakatni saqlab qolgan holda siyosat cheklovlari.
- Hisoblagichni saqlashni yetkazib berish (replikatsiya muvaffaqiyati, ish vaqti, yaxlitlik dalillari) va
  to'lovni taqsimlash uchun telemetriyani eksport qilish.
- Insofsiz provayderlar bo'lishi uchun bekor qilish va nizo jarayonlarini taqdim eting
  jazolanadi yoki olib tashlanadi.

## Domen tushunchalari

| Kontseptsiya | Tavsif | Dastlabki yetkazib beriladi |
|---------|-------------|---------------------|
| `CapacityDeclarationV1` | Norito foydali yuk provayder identifikatori, chunker profilini qoʻllab-quvvatlash, belgilangan GiB, qatorga xos cheklovlar, narx boʻyicha maslahatlar, staking majburiyati va amal qilish muddatini tavsiflaydi. | `sorafs_manifest::capacity` da sxema + validator. |
| `ReplicationOrder` | Bir yoki bir nechta provayderlarga manifest CID tayinlash, jumladan, ortiqchalik darajasi va SLA koʻrsatkichlari boʻyicha boshqaruv tomonidan berilgan koʻrsatma. | Norito sxemasi Torii + aqlli kontrakt API bilan ulashilgan. |
| `CapacityLedger` | Faol quvvat deklaratsiyasini, replikatsiya buyurtmalarini, ishlash ko'rsatkichlarini va to'lovni hisoblashni kuzatish zanjirdagi/zanjirdan tashqari reestr. | Aqlli kontrakt moduli yoki deterministik snapshotli tarmoqdan tashqari xizmat ko'rsatish stub. |
| `MarketplacePolicy` | Minimal ulush, audit talablari va jarima egri chizig'ini belgilovchi boshqaruv siyosati. | `sorafs_manifest` + boshqaruv hujjatidagi konfiguratsiya tuzilishi. |

### Amalga oshirilgan sxemalar (holat)

## Ish taqsimoti

### 1. Sxema va registr qatlami

| Vazifa | Ega(lar)i | Eslatmalar |
|------|----------|-------|
| `CapacityDeclarationV1`, `ReplicationOrderV1`, `CapacityTelemetryV1`ni aniqlang. | Saqlash jamoasi / Boshqaruv | Norito dan foydalaning; semantik versiyani va qobiliyatga havolalarni o'z ichiga oladi. |
| `sorafs_manifest` da parser + validator modullarini amalga oshiring. | Saqlash jamoasi | Monoton identifikatorlarni, sig'im chegaralarini, ulush talablarini qo'llash. |
| Har bir profil uchun `min_capacity_gib` bilan chunker registrining metamaʼlumotlarini kengaytiring. | Asboblar WG | Mijozlarga har bir profil uchun minimal apparat talablarini bajarishga yordam beradi. |
| Qabul qilish to'siqlari va jarimalar jadvalini aks ettiruvchi `MarketplacePolicy` hujjati loyihasi. | Boshqaruv Kengashi | Hujjatlarda standart sozlamalar bilan birga nashr qiling. |

#### Sxema ta'riflari (amalga oshirilgan)

- `CapacityDeclarationV1` har bir provayder uchun imzolangan sig'im majburiyatlarini, jumladan, kanonik chunker tutqichlari, qobiliyatga oid ma'lumotnomalar, ixtiyoriy qator chegaralari, narx bo'yicha maslahatlar, amal qilish oynalari va metama'lumotlarni qamrab oladi. Tasdiqlash nolga teng bo'lmagan ulush, kanonik tutqichlar, takrorlangan taxalluslar, e'lon qilingan jami doirasidagi har bir chiziqli bosh harflar va monoton GiB hisobini ta'minlaydi.【crates/sorafs_manifest/src/capacity.rs:28】
- `ReplicationOrderV1` manifestlarni ortiqcha maqsadlar, SLA chegaralari va har bir topshiriq kafolatlari bilan boshqaruv tomonidan berilgan topshiriqlarga bog'laydi; validatorlar Torii yoki reestr buyurtmani qabul qilgunga qadar kanonik chunker tutqichlari, noyob provayderlar va oxirgi muddat cheklovlarini amalga oshiradi.【crates/sorafs_manifest/src/capacity.rs:301】
- `CapacityTelemetryV1` toʻlov taqsimotini taʼminlovchi davr lavhalarini (eʼlon qilingan va ishlatilgan GiB, replikatsiya hisoblagichlari, ish vaqti/PoR foizlari) ifodalaydi. Chegara tekshiruvlari foydalanishni deklaratsiyalar ichida va foizlarda 0 – 100% ichida saqlaydi.【crates/sorafs_manifest/src/capacity.rs:476】
- Birgalikda yordamchilar (`CapacityMetadataEntry`, `PricingScheduleV1`, yoʻlak/tayinlash/SLA validatorlari) CI va quyi oqim vositalari qayta ishlatilishi mumkin boʻlgan deterministik kalit tekshiruvi va xato haqida hisobot beradi.【crates/sorafs_manifest/src/capacity.rs:230】
- `PinProviderRegistry` endi `/v2/sorafs/capacity/state` orqali provayder deklaratsiyasi va toʻlov kitobi yozuvlarini deterministik Norito orqasida birlashtirib, zanjirdagi suratni koʻrsatadi. JSON.【crates/iroha_torii/src/sorafs/registry.rs:17】【crates/iroha_torii/src/sorafs/api.rs:64】
- Validatsiya qamrovi kanonik qo'llashni amalga oshirish, dublikatlarni aniqlash, har bir qator chegaralari, replikatsiya tayinlash himoyasi va telemetriya diapazoni tekshiruvlarini o'tkazadi, shuning uchun regressiyalar darhol CIda paydo bo'ladi.【crates/sorafs_manifest/src/capacity.rs:792】
- Operator asboblari: `sorafs_manifest_stub capacity {declaration, telemetry, replication-order}` inson tomonidan o'qilishi mumkin bo'lgan spetsifikatsiyalarni kanonik Norito foydali yuklari, base64 bloblari va JSON xulosalariga aylantiradi, shuning uchun operatorlar `/v2/sorafs/capacity/declare`, `/v2/sorafs/capacity/telemetry` va replikatsiya tuzatish tartibini mahalliy tartibga solishlari mumkin. validation.【crates/sorafs_car/src/bin/sorafs_manifest_stub/capacity.rs:1】 Yoʻnaltiruvchi moslamalar `fixtures/sorafs_manifest/replication_order/` (`order_v1.json`, `order_v1.to`) da ishlaydi va I00.005 orqali yaratiladi.

### 2. Boshqaruv tekisligi integratsiyasi

| Vazifa | Ega(lar)i | Eslatmalar |
|------|----------|-------|
| Norito JSON foydali yuklari bilan `/v2/sorafs/capacity/declare`, `/v2/sorafs/capacity/telemetry`, `/v2/sorafs/capacity/orders` Torii ishlov beruvchilarini qo'shing. | Torii jamoasi | Mirror validator mantig'i; Norito JSON yordamchilaridan qayta foydalaning. |
| `CapacityDeclarationV1` oniy suratlarini orkestr jadvali metamaʼlumotlariga va shlyuzni olish rejalariga targʻib qiling. | Tooling WG / Orkestr jamoasi | `provider_metadata` ni sig'imga oid ma'lumotlar bilan kengaytiring, shuning uchun ko'p manbali ballar chiziq chegaralariga rioya qiladi. |
| Orkestrator/shlyuz mijozlariga topshiriqlar va ishlamay qolish bo'yicha maslahatlar berish uchun replikatsiya buyurtmalarini yuboring. | Networking TL / Gateway jamoasi | Scoreboard Builder boshqaruv tomonidan imzolangan replikatsiya buyurtmalarini ishlatadi. |
| CLI asboblari: `sorafs_cli`ni `capacity declare`, `capacity telemetry`, `capacity orders import` bilan kengaytiring. | Asboblar WG | Deterministik JSON + skorbord natijalarini taqdim eting. |

### 3. Bozor siyosati va boshqaruvi

| Vazifa | Ega(lar)i | Eslatmalar |
|------|----------|-------|
| `MarketplacePolicy` ni ratifikatsiya qiling (minimal ulush, jarima ko'paytmalari, audit kadensi). | Boshqaruv Kengashi | Hujjatlarda nashr qiling, tahrirlar tarixini yozib oling. |
| Parlament deklaratsiyalarni tasdiqlashi, yangilashi va bekor qilishi uchun boshqaruv ilgaklarini qo'shing. | Boshqaruv kengashi / Smart kontrakt jamoasi | Norito hodisalari + manifest qabul qilishdan foydalaning. |
| Telemetrli SLA buzilishi bilan bog'liq jarimalar jadvalini (to'lovni kamaytirish, obligatsiyalarni qisqartirish) amalga oshiring. | Boshqaruv Kengashi / G'aznachilik | `DealEngine` hisob-kitob chiqishlari bilan tekislang. |
| Hujjat nizolari jarayoni va eskalatsiya matritsasi. | Hujjatlar / Boshqaruv | Disput runbook + CLI yordamchilariga havola. |

### 4. Hisoblash va to'lovlarni taqsimlash

| Vazifa | Ega(lar)i | Eslatmalar |
|------|----------|-------|
| `CapacityTelemetryV1` ni qabul qilish uchun Torii oʻlchash kiritishni kengaytiring. | Torii jamoasi | GiB-soatlarni, PoR muvaffaqiyatini, ish vaqtini tasdiqlang. |
| Buyurtma bo'yicha foydalanish + SLA statistikasi haqida hisobot berish uchun `sorafs_node` o'lchash quvurini yangilang. | Saqlash jamoasi | Replikatsiya buyurtmalari va chunker tutqichlari bilan tekislang. |
| Hisob-kitob quvuri: telemetriya + replikatsiya ma'lumotlarini XOR-da belgilangan to'lovlarga aylantiring, boshqaruvga tayyor xulosalar chiqaring va daftar holatini qayd eting. | G'aznachilik / Saqlash jamoasi | Deal Engine / G'aznachilik eksportiga o'tkazing. |
| Sog'likni o'lchash uchun asboblar paneli/ogohlantirishlarni eksport qiling (qabul qilish to'plami, eski telemetriya). | Kuzatish mumkinligi | SF-6/SF-7 tomonidan havola qilingan Grafana paketini kengaytiring. |

- Torii endi `/v2/sorafs/capacity/telemetry` va `/v2/sorafs/capacity/state` (JSON + Norito) ochib beradi, shuning uchun operatorlar davr telemetriyasi suratlarini taqdim etishlari va inspektorlar auditorlik tekshiruvi yoki kanonik dalillarni olishlari mumkin. qadoqlash.【crates/iroha_torii/src/sorafs/api.rs:268】【crates/iroha_torii/src/sorafs/api.rs:816】
- `PinProviderRegistry` integratsiyasi replikatsiya buyurtmalariga bir xil so'nggi nuqta orqali kirishni ta'minlaydi; CLI yordamchilari (`sorafs_cli capacity telemetry --from-file telemetry.json`) endi deterministik xeshlash va taxallus ruxsati bilan avtomatlashtirishdan telemetriyani tasdiqlaydi/nashr qiladi.
- Oʻlchash oniy tasvirlari `metering` oniy tasviriga mahkamlangan `CapacityTelemetrySnapshot` yozuvlarini ishlab chiqaradi va Prometheus eksportlari importga tayyor Grafana platasini I18NI700B billing monitorida taʼminlaydi. real vaqtda hisoblangan nano-SORA toʻlovlari va SLA muvofiqligi.【crates/iroha_torii/src/routing.rs:5143】【docs/source/grafana_sorafs_metering.json:1】
- O'lchovlarni tekislash yoqilganda, surat `smoothed_gib_hours` va `smoothed_por_success_bps` ni o'z ichiga oladi, shuning uchun operatorlar EMA trendidagi qiymatlarni boshqaruv to'lovlar uchun ishlatadigan xom hisoblagichlar bilan solishtirishi mumkin.【crates/sorafs_node/src/metering.rs:401

### 5. Nizolar va bekor qilishni ko'rib chiqish

| Vazifa | Ega(lar)i | Eslatmalar |
|------|----------|-------|
| `CapacityDisputeV1` foydali yukini aniqlang (shikoyatchi, dalil, maqsadli provayder). | Boshqaruv Kengashi | Norito sxemasi + validator. |
| CLI nizolarni topshirish va javob berish uchun qo'llab-quvvatlash (dalil qo'shimchalari bilan). | Asboblar WG | Dalillar to'plamining deterministik xeshlanishini ta'minlang. |
| Qayta-qayta SLA buzilishi uchun avtomatlashtirilgan tekshiruvlarni qo'shing (nizoni avtomatik ravishda oshirish). | Kuzatish mumkinligi | Ogohlantirish chegaralari va boshqaruv ilgaklari. |
| Hujjatni bekor qilish kitobi (imtiyozli davr, mahkamlangan ma'lumotlarni evakuatsiya qilish). | Hujjatlar / Saqlash jamoasi | Politsiya hujjati va operatorning ish kitobiga havola. |

## Sinov va CI talablari- Barcha yangi sxema tekshiruvchilari uchun birlik testlari (`sorafs_manifest`).
- Simulyatsiya qiluvchi integratsiya testlari: deklaratsiya → replikatsiya tartibi → o'lchash → to'lov.
- Namuna sig'imi deklaratsiyasini/temetriyani qayta tiklash va imzolarning sinxron bo'lishini ta'minlash uchun CI ish jarayoni (`ci/check_sorafs_fixtures.sh`ni kengaytiring).
- API ro'yxatga olish kitobi uchun testlarni yuklash (10 ming provayder, 100 ming buyurtma simulyatsiyasi).

## Telemetriya va asboblar paneli

- asboblar paneli panellari:
  - E'lon qilingan sig'im va provayder uchun ishlatilgan.
  - Replikatsiya buyurtmasining kechikishi va topshiriqning o'rtacha kechikishi.
  - SLA muvofiqligi (ish vaqti %, PoR muvaffaqiyat darajasi).
  - Har bir davr uchun to'lov va jarimalar.
- Ogohlantirishlar:
  - Provayder minimal majburiy quvvatdan past.
  - Replikatsiya tartibi tiqilib qoldi > SLA.
  - o'lchash quvurlarining nosozliklari.

## Hujjatlarni yetkazib berish

- Imkoniyatlarni e'lon qilish, majburiyatlarni yangilash va foydalanish monitoringi bo'yicha operator qo'llanmasi.
- Deklaratsiyalarni tasdiqlash, farmoyishlar chiqarish, nizolarni ko'rib chiqish bo'yicha boshqaruv yo'riqnomasi.
- Imkoniyatlar so'nggi nuqtalari va replikatsiya tartibi formati uchun API ma'lumotnomasi.
- Ishlab chiquvchilar uchun bozorda tez-tez so'raladigan savollar.

## GA tayyorligini tekshirish roʻyxati

“Yo‘l xaritasi” bandi **SF-2c** buxgalteriya hisobi bo‘yicha aniq dalillar asosida ishlab chiqarishni yo‘lga qo‘yadi,
nizolarni ko'rib chiqish va ishga tushirish. Qabul qilish mezonlarini saqlash uchun quyidagi artefaktlardan foydalaning
amalga oshirish bilan hamohang.

### Kecha buxgalteriya hisobi va XORni solishtirish
- Xuddi shu oyna uchun sig'im holati oniy rasmini va XOR daftarining eksportini eksport qiling, so'ngra ishga tushiring:
  ```bash
  python3 scripts/telemetry/capacity_reconcile.py \
    --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
    --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
    --label nightly-capacity \
    --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
    --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
  ```
  Yordamchi etishmayotgan/ortiqcha to'langan hisob-kitoblar yoki jarimalar bo'yicha noldan farq qiladi va Prometheus chiqaradi
  matn fayli xulosasi.
- Ogohlantirish `SoraFSCapacityReconciliationMismatch` (`dashboards/alerts/sorafs_capacity_rules.yml` da)
  yarashtirish ko'rsatkichlari bo'shliqlar haqida xabar berganda yong'in chiqadi; asboblar paneli ostida yashaydi
  `dashboards/grafana/sorafs_capacity_penalties.json`.
- `docs/examples/sorafs_capacity_marketplace_validation/` ostida JSON xulosasi va xeshlarni arxivlang
  boshqaruv paketlari bilan bir qatorda.

### Bahs va dalillarni kesish
- `sorafs_manifest_stub capacity dispute` orqali nizolarni hal qiling (testlar:
  `cargo test -p sorafs_car --test capacity_cli`) shuning uchun foydali yuklar kanonik bo'lib qoladi.
- `cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic` va jarimani ishga tushiring
  suites (`record_capacity_telemetry_penalises_persistent_under_delivery`) nizolarni isbotlash va
  slashlar deterministik tarzda takrorlanadi.
- Dalillarni qo'lga kiritish va kuchaytirish uchun `docs/source/sorafs/dispute_revocation_runbook.md` ga rioya qiling;
  e'tiroz tasdiqlarini tekshirish hisobotiga qaytaring.

### Provayderni ishga tushirish va tutundan chiqish sinovlari
- `sorafs_manifest_stub capacity ...` bilan deklaratsiya/temetriya artefaktlarini qayta tiklang va takrorlang
  topshirishdan oldin CLI sinovlari (`cargo test -p sorafs_car --test capacity_cli -- capacity_declaration`).
- Torii (`/v2/sorafs/capacity/declare`) orqali yuboring, keyin `/v2/sorafs/capacity/state` plyusni oling
  Grafana skrinshotlari. `docs/source/sorafs/capacity_onboarding_runbook.md` da chiqish oqimiga rioya qiling.
- Ichkarida imzolangan artefaktlar va yarashuv natijalarini arxivlash
  `docs/examples/sorafs_capacity_marketplace_validation/`.

## Bog'liqlar va ketma-ketlik

1. SF-2b ni tugatish (qabul qilish siyosati) - bozor tekshirilgan provayderlarga tayanadi.
2. Torii integratsiyasidan oldin sxema + registr qatlamini (ushbu hujjat) amalga oshiring.
3. To'lovlarni yoqishdan oldin o'lchash quvurini to'liq bajaring.
4. Yakuniy bosqich: o'lchash ma'lumotlari bosqichma-bosqich tekshirilgandan so'ng, boshqaruv tomonidan boshqariladigan to'lovlarni taqsimlashni yoqing.

Taraqqiyot ushbu hujjatga havolalar bilan yo'l xaritasida kuzatilishi kerak. Har bir asosiy bo'lim (sxema, boshqaruv tekisligi, integratsiya, o'lchash, nizolarni hal qilish) to'liq xususiyatga erishgandan so'ng, yo'l xaritasini yangilang.