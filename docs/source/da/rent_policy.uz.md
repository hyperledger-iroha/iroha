---
lang: uz
direction: ltr
source: docs/source/da/rent_policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c7cdc46bcd87af7924817a94900c8fad2c23570607f4065f19d8a42d259fe83f
source_last_modified: "2026-01-22T14:35:37.691079+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Ma'lumotlar mavjudligini ijaraga olish va rag'batlantirish siyosati (DA-7)

_Holat: Loyihalash — Egalari: Iqtisodiyot GG / G'aznachilik / Saqlash jamoasi_

“Yo‘l xaritasi” bandi **DA-7** har bir blob uchun XOR bilan belgilangan ijara haqini taqdim etadi.
`/v1/da/ingest` ga taqdim etilgan, shuningdek, PDP/PoTR bajarilishini mukofotlaydigan bonuslar va
chiqish mijozlarni olish uchun xizmat qildi. Ushbu hujjat dastlabki parametrlarni belgilaydi,
ularning ma'lumotlar modeli tasviri va Torii tomonidan qo'llaniladigan hisoblash ish jarayoni,
SDK va G'aznachilik boshqaruv paneli.

## Siyosat tuzilishi

Siyosat [`DaRentPolicyV1`](/crates/iroha_data_model/src/da/types.rs) sifatida kodlangan
ma'lumotlar modeli doirasida. Torii va boshqaruv vositalari siyosatni davom ettirmoqda
Norito foydali yuklar, shuning uchun ijara narxlari va rag'batlantiruvchi daftarlarni qayta hisoblash mumkin
deterministik tarzda. Sxema beshta tugmachani ko'rsatadi:

| Maydon | Tavsif | Standart |
|-------|-------------|---------|
| `base_rate_per_gib_month` | XOR har oy saqlash uchun GiB uchun to‘lanadi. | `250_000` micro-XOR (0,25 XOR) |
| `protocol_reserve_bps` | Protokol zaxirasiga yo'naltirilgan ijara ulushi (asosiy punktlar). | `2_000` (20%) |
| `pdp_bonus_bps` | Muvaffaqiyatli PDP baholash uchun bonus ulushi. | `500` (5%) |
| `potr_bonus_bps` | Muvaffaqiyatli PoTR baholash uchun bonus ulushi. | `250` (2,5%) |
| `egress_credit_per_gib` | Provayder 1GiB DA maʼlumotlariga xizmat qilganda toʻlangan kredit. | `1_500` micro-XOR |

Barcha bazaviy qiymatlar `BASIS_POINTS_PER_UNIT` (10000) ga nisbatan tasdiqlangan.
Siyosat yangilanishlari boshqaruv orqali o'tishi kerak va har bir Torii tugunida
`torii.da_ingest.rent_policy` konfiguratsiya bo'limi orqali faol siyosat
(`iroha_config`). Operatorlar `config.toml` da standart sozlamalarni bekor qilishi mumkin:

```toml
[torii.da_ingest.rent_policy]
base_rate_per_gib_month_micro = 250000        # 0.25 XOR/GiB-month
protocol_reserve_bps = 2000                   # 20% protocol reserve
pdp_bonus_bps = 500                           # 5% PDP bonus
potr_bonus_bps = 250                          # 2.5% PoTR bonus
egress_credit_per_gib_micro = 1500            # 0.0015 XOR/GiB egress credit
```

CLI asboblari (`iroha app da rent-quote`) bir xil Norito/JSON siyosati kiritishlarini qabul qiladi
va faol `DaRentPolicyV1` ga yetmasdan aks ettiruvchi artefaktlarni chiqaradi
Torii holatiga qayting. Yutish uchun ishlatiladigan siyosat snapshotini taqdim eting
kotirovka takrorlanishi mumkin bo'lib qoladi.

### Doimiy ijara kotirovkalari artefaktlari

`iroha app da rent-quote --gib <size> --months <months> --quote-out <path>` ni ishga tushiring
ekrandagi xulosani ham, chiroyli chop etilgan JSON artefaktini ham chiqaradi. Fayl
yozuvlar `policy_source`, ichkariga kiritilgan `DaRentPolicyV1` surati, hisoblangan
`DaRentQuote` va olingan `ledger_projection` (seriyali
[`DaRentLedgerProjection`](/crates/iroha_data_model/src/da/types.rs)) uni xazina panellari va ISI daftarlari uchun mos qiladi.
quvurlar. `--quote-out` o'rnatilgan katalogga ishora qilganda CLI istalgan katalogni yaratadi.
yo'qolgan ota-onalar, shuning uchun operatorlar kabi joylarni standartlashtirishi mumkin
Boshqa DA dalillar to'plamlari bilan bir qatorda `artifacts/da/rent_quotes/<timestamp>.json`.
Artefaktni XORni ijaraga olish uchun tasdiqlash yoki yarashtirish uchun biriktiring
taqsimot (asosiy ijara, zaxira, PDP/PoTR bonuslari va chiqish kreditlari).
takrorlanishi mumkin. Avtomatik ravishda bekor qilish uchun `--policy-label "<text>"` dan o'ting
olingan `policy_source` tavsifi (fayl yo'llari, o'rnatilgan standart va boshqalar) bilan
boshqaruv chiptasi yoki manifest xesh kabi inson tomonidan o'qiladigan teg; CLI kesishadi
bu qiymat va bo'sh/faqat bo'sh bo'shliqlarni rad etadi, shuning uchun yozilgan dalillar
tekshirilishi mumkin bo'lib qoladi.

```json
{
  "policy_source": "policy JSON `configs/da/rent_policy.json`",
  "gib": 10,
  "months": 3,
  "policy": { "...": "DaRentPolicyV1 fields elided" },
  "quote": { "...": "DaRentQuote breakdown" },
  "ledger_projection": {
    "rent_due": { "micro": 7500000 },
    "protocol_reserve_due": { "micro": 1500000 },
    "provider_reward_due": { "micro": 6000000 },
    "pdp_bonus_pool": { "micro": 375000 },
    "potr_bonus_pool": { "micro": 187500 },
    "egress_credit_per_gib": { "micro": 1500 }
  }
}
```Buxgalteriya hisobining proyeksiyasi bo'limi to'g'ridan-to'g'ri DA ijara kitobi ISIga kiradi: u
protokol zaxirasi, provayder to'lovlari va uchun mo'ljallangan XOR deltalarini belgilaydi
maxsus orkestrlash kodini talab qilmasdan, har bir isbotlangan bonusli hovuzlar.

### Ijara kitobi rejalarini yaratish

`iroha app da rent-ledger --quote <path> --payer-account <id> --treasury-account <id> --protocol-reserve-account <id> --provider-account <id> --pdp-bonus-account <id> --potr-bonus-account <id> --asset-definition 61CtjvNd9T3THAR65GsMVHr82Bjc` ishga tushiring
doimiy ijara kotirovkasini bajariladigan daftar o'tkazmalariga aylantirish uchun. Buyruq
o'rnatilgan `ledger_projection` ni tahlil qiladi, Norito `Transfer` ko'rsatmalarini chiqaradi
asosiy ijara haqini xazinaga yig'ib, zaxirani/provayderni yo'naltiradi
qismlarga ajratadi va PDP/PoTR bonus pullarini to'g'ridan-to'g'ri to'lovchidan oldindan moliyalashtiradi. The
JSON chiqishi kotirovka metamaʼlumotlarini aks ettiradi, shuning uchun CI va gʻaznachilik vositalari sabab boʻlishi mumkin
Xuddi shu artefakt haqida:

```json
{
  "quote_path": "artifacts/da/rent_quotes/2025-12-07/rent.json",
  "rent_due_micro_xor": 7500000,
  "protocol_reserve_due_micro_xor": 1500000,
  "provider_reward_due_micro_xor": 6000000,
  "pdp_bonus_pool_micro_xor": 375000,
  "potr_bonus_pool_micro_xor": 187500,
  "egress_credit_per_gib_micro_xor": 1500,
  "instructions": [
    { "Transfer": { "...": "payer -> treasury base rent instruction elided" }},
    { "Transfer": { "...": "treasury -> reserve" }},
    { "Transfer": { "...": "treasury -> provider payout" }},
    { "Transfer": { "...": "payer -> PDP bonus escrow" }},
    { "Transfer": { "...": "payer -> PoTR bonus escrow" }}
  ]
}
```

Yakuniy `egress_credit_per_gib_micro_xor` maydoni asboblar paneli va to'lovni amalga oshirish imkonini beradi
rejalashtirishchilar chiqish to'lovlarini ishlab chiqarilgan ijara siyosati bilan moslashtiradi
skript elimidagi siyosat matematikasini qayta hisoblamasdan iqtibos keltiring.

## Iqtibosga misol

```rust
use iroha_data_model::da::types::DaRentPolicyV1;

// 10 GiB retained for 3 months.
let policy = DaRentPolicyV1::default();
let quote = policy.quote(10, 3).expect("policy validated");

assert_eq!(quote.base_rent.as_micro(), 7_500_000);      // 7.5 XOR total rent
assert_eq!(quote.protocol_reserve.as_micro(), 1_500_000); // 20% reserve
assert_eq!(quote.provider_reward.as_micro(), 6_000_000);  // Direct provider payout
assert_eq!(quote.pdp_bonus.as_micro(), 375_000);          // PDP success bonus
assert_eq!(quote.potr_bonus.as_micro(), 187_500);         // PoTR success bonus
assert_eq!(quote.egress_credit_per_gib.as_micro(), 1_500);
```

Kotirovka Torii tugunlari, SDKlar va G'aznachilik hisobotlarida takrorlanishi mumkin, chunki
u vaqtinchalik matematik o'rniga deterministik Norito tuzilmalaridan foydalanadi. Operatorlar mumkin
boshqaruv takliflari yoki ijaraga JSON/CBOR kodli `DaRentPolicyV1` ilovasini biriktiring
har qanday blok uchun qaysi parametrlar amal qilganligini isbotlash uchun auditlar.

## Bonuslar va zaxiralar

- **Protokol zaxirasi:** `protocol_reserve_bps` qo'llab-quvvatlaydigan XOR zaxirasini moliyalashtiradi
  favqulodda qayta takrorlash va to'lovlarni qisqartirish. G'aznachilik bu chelakni kuzatib boradi
  Buxgalteriya hisobidagi qoldiqlar sozlangan stavkaga mos kelishini ta'minlash uchun alohida.
- **PDP/PoTR bonuslari:** Har bir muvaffaqiyatli dalil bahosi qo'shimcha mukofot oladi
  to'lov `base_rent × bonus_bps` dan olingan. DA rejalashtiruvchisi dalillarni chiqaradi
  kvitansiyalar unda rag'batlarni takrorlash mumkin bo'lgan asosiy nuqta teglari mavjud.
- **Chiqish krediti:** Provayderlar har bir manifestda taqdim etilgan GiBni qayd qiladi, ga ko'paytiradi
  `egress_credit_per_gib` va kvitansiyalarni `iroha app da prove-availability` orqali yuboring.
  Ijara siyosati har bir GiB miqdorini boshqaruv bilan hamohang ushlab turadi.

## Operatsion oqim

1. **Ingest:** `/v1/da/ingest` faol `DaRentPolicyV1` yuklaydi, kotirovkalarni ijaraga oladi
   blob hajmi va saqlanishiga asoslanadi va kotirovkani Norito ichiga kiritadi
   namoyon. Yuboruvchi ijara xashiga ishora qiluvchi bayonotga imzo chekadi va
   saqlash chiptasi identifikatori.
2. **Buxgalteriya:** G'aznachilik ingest skriptlari manifestni dekodlaydi, qo'ng'iroq qiladi
   `DaRentPolicyV1::quote` va ijara daftarlarini to'ldiring (asosiy ijara, zaxira,
   bonuslar va kutilgan chiqish kreditlari). Ro'yxatga olingan ijara o'rtasidagi har qanday tafovut
   va qayta hisoblangan kotirovkalar CI muvaffaqiyatsizlikka uchraydi.
3. **Tasdiqlovchi mukofotlar:** PDP/PoTR rejalashtiruvchilari muvaffaqiyatga erishganlarida ular kvitansiya chiqaradilar.
   manifest dayjest, isbot turi va XOR bonusidan olingan
   siyosat. Boshqaruv bir xil taklifni qayta hisoblash orqali to'lovlarni tekshirishi mumkin.
4. **Chiqish toʻlovi:** Olib tashlash orkestrlari imzolangan chiqish xulosalarini taqdim etadilar.
   Torii GiB sonini `egress_credit_per_gib` ga ko'paytiradi va to'lovni chiqaradi
   ijara eskroviga qarshi ko'rsatmalar.

## TelemetriyaTorii tugunlari ijaradan foydalanishni quyidagi Prometheus koʻrsatkichlari (yorliqlar:
`cluster`, `storage_class`):

- `torii_da_rent_gib_months_total` — GiB-oylar `/v1/da/ingest` tomonidan keltirilgan.
- `torii_da_rent_base_micro_total` — qabul qilinganda hisoblangan asosiy ijara (mikro XOR).
- `torii_da_protocol_reserve_micro_total` — protokol zahirasi badallari.
- `torii_da_provider_reward_micro_total` - provayder tomonidan ijara to'lovlari.
- `torii_da_pdp_bonus_micro_total` va `torii_da_potr_bonus_micro_total` -
  PDP/PoTR bonus pullari qabul qilingan kotirovkadan olingan.

Iqtisodiyot asboblar paneli bu hisoblagichlarga tayanadi, buxgalteriya ISI, zaxira kranlar,
va PDP/PoTR bonus jadvallari har biri uchun amaldagi siyosat parametrlariga mos keladi
klaster va saqlash klassi. SoraFS Capacity Health Grafana platasi
(`dashboards/grafana/sorafs_capacity_health.json`) endi maxsus panellarni taqdim etadi
ijara taqsimoti uchun, PDP/PoTR bonus hisoblash, va GiB-oy qo'lga, ruxsat
G'aznachilik yutishni ko'rib chiqishda Torii klasteri yoki saqlash klassi bo'yicha filtrlanadi
hajmi va to'lovlari.

## Keyingi qadamlar

- ✅ `/v1/da/ingest` kvitansiyalari endi `rent_quote` ni o'z ichiga oladi va CLI/SDK sirtlarida ko'rsatilgan ko'rsatilgan
  asosiy ijara, zaxira ulush va PDP/PoTR bonuslari, shuning uchun topshiruvchilar XOR majburiyatlarini oldindan ko'rib chiqishlari mumkin.
  foydali yuklarni qabul qilish.
- Ijara kitobini kelgusida DA obro'si/buyurtma kitobi tasmasi bilan integratsiyalash
  yuqori darajadagi provayderlar to'g'ri to'lovlarni olayotganligini isbotlash.