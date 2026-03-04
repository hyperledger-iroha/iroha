---
lang: uz
direction: ltr
source: docs/portal/docs/sorafs/migration-roadmap.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 84fdf4ef66efbbf7adef9cfaf41be0817ac656bec010c0a816d1f5a5310f1875
source_last_modified: "2026-01-05T09:28:11.885227+00:00"
translation_last_reviewed: 2026-02-07
title: "SoraFS Migration Roadmap"
translator: machine-google-reviewed
---

> [`docs/source/sorafs/migration_roadmap.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/migration_roadmap.md) dan moslashtirilgan.

# SoraFS Migratsiya yoʻl xaritasi (SF-1)

Ushbu hujjatda olingan migratsiya yo'riqnomasi amal qiladi
`docs/source/sorafs_architecture_rfc.md`. U SF-1 yetkazib berish imkoniyatlarini kengaytiradi
bajarishga tayyor bosqichlar, o'tish mezonlari va egasining nazorat ro'yxati, shuning uchun saqlash,
SoraFS tomonidan qo'llab-quvvatlanadigan nashrga mezbonlik qiluvchi artefakt.

Yo'l xaritasi qasddan deterministikdir: har bir bosqich kerakli narsani belgilaydi
artefaktlar, buyruq chaqiruvlari va attestatsiya bosqichlari shunday quyi oqim quvurlari
bir xil natijalarni ishlab chiqaradi va boshqaruv tekshirilishi mumkin bo'lgan izni saqlab qoladi.

## Muhim bosqichga umumiy nuqtai

| Muhim bosqich | Oyna | Asosiy maqsadlar | Yetkazib berish kerak | Egalari |
|----------|--------|---------------|-----------|--------|
| **M1 – Deterministik ijro** | 7–12 haftalar | Imzolangan armatura va sahna taxalluslarini tasdiqlang, quvurlar esa kutish bayroqlarini qabul qiladi. | Tungi fikstürni tekshirish, kengash imzolagan manifestlar, taxallus ro'yxatga olish kitobi yozuvlari. | Saqlash, boshqaruv, SDK |

Muhim bosqich holati `docs/source/sorafs/migration_ledger.md` da kuzatiladi. Hammasi
ushbu yo'l xaritasiga kiritilgan o'zgartirishlar boshqaruvni saqlash va chiqarish uchun buxgalteriya kitobini yangilashi KERAK
muhandislik hamohang.

## Ish oqimlari

### 2. Deterministik pinning qabul qilish

| Qadam | Muhim bosqich | Tavsif | Ega(lar)i | Chiqish |
|------|-----------|-------------|----------|--------|
| Armatura mashqlari | M0 | `fixtures/sorafs_chunker` bilan mahalliy chunk digestlarni taqqoslaydigan haftalik quruq yugurishlar. `docs/source/sorafs/reports/` ostida hisobotni nashr qilish. | Saqlash provayderlari | `determinism-<date>.md` o'tish/qobiliyatsiz matritsasi bilan. |
| Imzolarni majburlash | M1 | Imzolar yoki manifest drift bo'lsa, `ci/check_sorafs_fixtures.sh` + `.github/workflows/sorafs-fixtures-nightly.yml` bajarilmaydi. Rivojlanishni bekor qilish PRga biriktirilgan boshqaruvdan voz kechishni talab qiladi. | Asboblar WG | CI jurnali, voz kechish chiptasi havolasi (agar mavjud bo'lsa). |
| Kutish bayroqlari | M1 | Quvur quvurlari `sorafs_manifest_stub` ga qo'ng'iroq qiladi va chiqishlarni aniq kutadi: | Docs CI | Kutish bayroqlariga havola qiluvchi yangilangan skriptlar (quyidagi buyruq blokiga qarang). |
| Ro'yxatga olish kitobida birinchi marta mahkamlash | M2 | `sorafs pin propose` va `sorafs pin approve` manifest taqdimotlarini o'rash; CLI standarti `--require-registry`. | Boshqaruv operatsiyalari | Registry CLI audit jurnali, muvaffaqiyatsiz takliflar uchun telemetriya. |
| Kuzatilish pariteti | M3 | Prometheus/Grafana asboblar paneli ro'yxatga olish kitobi manifestlaridan bo'lak to'plamlari ajralib chiqqanda ogohlantiradi; ogohlantirishlar qo'ng'iroq bo'yicha operatsiyalarga uzatiladi. | Kuzatish mumkinligi | Boshqaruv paneli havolasi, ogohlantirish qoida identifikatorlari, GameDay natijalari. |

#### Kanonik nashriyot buyrug'i

```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- docs/book \
  --manifest-out artifacts/docs/book/2025-11-01/docs.manifest \
  --manifest-signatures-out artifacts/docs/book/2025-11-01/docs.manifest_signatures.json \
  --car-out artifacts/docs/book/2025-11-01/docs.car \
  --chunk-fetch-plan-out artifacts/docs/book/2025-11-01/docs.fetch_plan.json \
  --car-digest=13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482 \
  --car-size=429391872 \
  --root-cid=f40101... \
  --dag-codec=0x71
```

Dijest, hajm va CID qiymatlarini kutilgan havolalar bilan almashtiring
artefakt uchun migratsiya kitobi yozuvi.

### 3. Alias Transition & Communications

| Qadam | Muhim bosqich | Tavsif | Ega(lar)i | Chiqish |
|------|-----------|-------------|----------|--------|
| Sahnalashtirishda taxallus isbotlari | M1 | Pin Registry staging muhitida taxallusga da'volarni ro'yxatdan o'tkazing va manifestlarga Merkle dalillarini qo'shing (`--alias`). | Boshqaruv, Hujjatlar | Taxallus nomi bilan manifest + daftar sharhi yonida saqlangan isbot to'plami. |
| Tasdiqlash ijrosi | M2 | Shlyuzlar yangi `Sora-Proof` sarlavhalarisiz manifestlarni rad etadi; CI dalillarni olish uchun `sorafs alias verify` qadamini oladi. | Tarmoqlar | Gateway konfiguratsiya yamogʻi + CI chiqishi tasdiqlanishi muvaffaqiyati. |

### 4. Aloqa va audit

- **Buxgalteriya kitobi intizomi:** har bir holat o'zgarishi (fiksatorning o'zgarishi, ro'yxatga olish kitobini topshirish,
  taxallusni faollashtirish) sanasi ko'rsatilgan eslatmani qo'shishi kerak
  `docs/source/sorafs/migration_ledger.md`.
- **Boshqaruv bayonnomasi:** pin reestridagi o'zgarishlarni tasdiqlovchi kengash sessiyalari yoki
  taxallus siyosatlari ham ushbu yoʻl xaritasiga, ham daftarga havola qilishi kerak.
- **Tashqi xabarlar:** DevRel har bir bosqichda holat yangilanishlarini e'lon qiladi (blog +
  o'zgarishlar jurnalidan parcha) deterministik kafolatlar va taxallus vaqt jadvallarini ta'kidlaydi.

## Bog'liqlar va xavflar

| Bog'liqlik | Ta'sir | Yumshatish |
|------------|--------|------------|
| Pin Registry shartnoma mavjudligi | M2 pin-birinchi chiqishni bloklaydi. | Takroriy sinovlar bilan M2 oldidan bosqichli shartnoma; regressiyasiz bo'lgunga qadar konvertni qayta tiklash. |
| Kengash imzolash kalitlari | Manifest konvertlari va reestrni tasdiqlash uchun talab qilinadi. | Imzolash marosimi `docs/source/sorafs/signing_ceremony.md` da hujjatlashtirilgan; tugmachalarni bir-biriga yopishgan va daftar yozuvi bilan aylantiring. |
| SDK reliz kadansi | Mijozlar M3 dan oldin taxallusni isbotlashlari kerak. | SDK chiqarish oynalarini muhim eshiklar bilan tekislang; shablonlarni chiqarish uchun migratsiya nazorat ro'yxatlarini qo'shing. |

Qolgan xavflar va kamaytirish choralari `docs/source/sorafs_architecture_rfc.md` da aks ettirilgan
va tuzatishlar kiritilganda o'zaro bog'liq bo'lishi kerak.

## Chiqish mezonlari nazorat roʻyxati

| Muhim bosqich | Mezonlar |
|----------|----------|
| M1 | - Ketma-ket etti kun davomida yashil tungi armatura ish. <br /> - CI da tasdiqlangan bosqichli taxallus isbotlari. <br /> - Boshqaruv kutilayotgan bayroq siyosatini ratifikatsiya qiladi. |

## O'zgarishlarni boshqarish

1. Ushbu faylni yangilash orqali PR orqali tuzatishlarni taklif qiling **va**
   `docs/source/sorafs/migration_ledger.md`.
2. PR tavsifida boshqaruv protokollari va CI dalillarini qo'llab-quvvatlovchi havola.
3. Birlashganda, xotira + DevRel pochta ro'yxatini xulosa va kutilgan holda xabardor qiling
   operator harakatlari.

Ushbu protseduradan so'ng SoraFS chiqarilishi deterministik bo'lib qoladi,
Nexus ishga tushirilishida ishtirok etuvchi jamoalar orasida tekshiriladigan va shaffof.