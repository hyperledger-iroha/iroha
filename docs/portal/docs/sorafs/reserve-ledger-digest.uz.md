---
lang: uz
direction: ltr
source: docs/portal/docs/sorafs/reserve-ledger-digest.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f0e832438aa843ed1ce6d018055a67bd1688ccb4cead9f73b87033469256015d
source_last_modified: "2026-01-22T16:26:46.526983+00:00"
translation_last_reviewed: 2026-02-07
title: Reserve Ledger Digest & Dashboards
description: How to turn `sorafs reserve ledger` output into telemetry, dashboards, and alerts for the Reserve+Rent policy.
translator: machine-google-reviewed
---

Zaxira+Ijara siyosati (yo‘l xaritasi **SFM‑6**) endi `sorafs reserve` yetkazib beradi
CLI yordamchilari va `scripts/telemetry/reserve_ledger_digest.py` tarjimoni shunday
g'aznachilik deterministik ijara/zaxira o'tkazmalarini chiqarishi mumkin. Bu sahifa aks etadi
ish jarayoni `docs/source/sorafs_reserve_rent_plan.md` da belgilangan va tushuntiradi
Grafana + Alertmanager-ga yangi transfer tasmasini qanday ulash kerak, shuning uchun iqtisodiyot va
boshqaruv tekshiruvchilari har bir hisob-kitob davrini tekshirishlari mumkin.

## Oxir-oqibat ish jarayoni

1. **Iqtibos + daftar proyeksiyasi**
   ```bash
   sorafs reserve quote \
     --storage-class hot \
     --tier tier-a \
     --duration monthly \
     --gib 250 \
     --quote-out artifacts/sorafs_reserve/quotes/provider-alpha-apr.json

  sorafs reserve ledger \
    --quote artifacts/sorafs_reserve/quotes/provider-alpha-apr.json \
    --provider-account soraカタカナ... \
    --treasury-account soraカタカナ... \
    --reserve-account soraカタカナ... \
    --asset-definition 61CtjvNd9T3THAR65GsMVHr82Bjc \
    --json-out artifacts/sorafs_reserve/ledger/provider-alpha-apr.json
   ```
   Buxgalteriya kitobi yordamchisi `ledger_projection` blokini biriktiradi (ijara muddati, zaxira
   taqchillik, to'ldirish deltasi, anderrayting mantiqiy qiymatlari) va Norito `Transfer`
   ISIlar XORni g'aznachilik va zaxira hisoblari o'rtasida ko'chirish uchun kerak edi.

2. **Dajest + Prometheus/NDJSON natijalarini yarating**
   ```bash
   python3 scripts/telemetry/reserve_ledger_digest.py \
     --ledger artifacts/sorafs_reserve/ledger/provider-alpha-apr.json \
     --label provider-alpha-apr \
     --out-json artifacts/sorafs_reserve/ledger/provider-alpha-apr.digest.json \
     --out-md docs/source/sorafs/reports/provider-alpha-apr-ledger.md \
     --ndjson-out artifacts/sorafs_reserve/ledger/provider-alpha-apr.ndjson \
     --out-prom artifacts/sorafs_reserve/ledger/provider-alpha-apr.prom
   ```
   Digest yordamchisi mikro-XOR yig'indisini XORga normallashtiradi, buning mavjudligini qayd qiladi
   proyeksiya anderraytingga javob beradi va **transfer tasmasi** ko‘rsatkichlarini chiqaradi
   `sorafs_reserve_ledger_transfer_xor` va
   `sorafs_reserve_ledger_instruction_total`. Bir nechta daftar bo'lishi kerak bo'lganda
   qayta ishlangan (masalan, provayderlar to'plami), `--ledger`/`--label` juftlarini takrorlang va
   yordamchi har bir dayjestni o'z ichiga olgan bitta NDJSON/Prometheus faylini yozadi.
   asboblar paneli butun tsiklni buyurtma elimsiz yutadi. `--out-prom`
   fayl tugunni eksport qiluvchi matn fayli kollektoriga mo'ljallangan - `.prom` faylini
   eksportyorning kuzatilgan katalogini yoki uni telemetriya paqiriga yuklang
   Zaxira asboblar paneli ishi tomonidan iste'mol qilinadi - `--ndjson-out` esa xuddi shunday ta'minlaydi
   ma'lumotlar quvurlariga foydali yuklar.

3. **Artefaktlar + dalillarni nashr qilish**
   - Dijestlarni `artifacts/sorafs_reserve/ledger/<provider>/` ostida saqlang va havola qiling
     haftalik iqtisodiy hisobotingizdan Markdown xulosasi.
   - JSON dayjestini ijara to'lovining yoqib yuborilishiga ilova qiling (shunda auditorlar
     matematika) va nazorat summasini boshqaruv dalillari paketiga kiriting.
   - Agar dayjest to'ldirish yoki anderrayting buzilishi haqida signal bersa, ogohlantirishga murojaat qiling
     Identifikatorlar (`SoraFSReserveLedgerTopUpRequired`,
     `SoraFSReserveLedgerUnderwritingBreach`) va qaysi ISIlar oʻtkazilayotganiga eʼtibor bering
     qo'llaniladi.

## Ko'rsatkichlar → asboblar paneli → ogohlantirishlar

| Manba metrikasi | Grafana paneli | Ogohlantirish / siyosat ilgagi | Eslatmalar |
|-------------|---------------|---------------------|-------|
| `torii_da_rent_base_micro_total`, `torii_da_protocol_reserve_micro_total`, `torii_da_provider_reward_micro_total` | `dashboards/grafana/sorafs_capacity_health.json` da “DA ijara taqsimoti (XOR/soat)” | Haftalik xazina dayjestini boqing; zahira oqimidagi keskinliklar `SoraFSCapacityPressure` (`dashboards/alerts/sorafs_capacity_rules.yml`) ga tarqaladi. |
| `torii_da_rent_gib_months_total` | “Imkoniyatlardan foydalanish (GiB-oylar)” (xuddi shu asboblar paneli) | Hisob-fakturani saqlash XOR o'tkazmalariga mos kelishini isbotlash uchun buxgalteriya dayjesti bilan bog'lang. |
| `sorafs_reserve_ledger_rent_due_xor`, `sorafs_reserve_ledger_reserve_shortfall_xor`, `sorafs_reserve_ledger_top_up_shortfall_xor` | “Reserve Snapshot (XOR)” + status kartalari `dashboards/grafana/sorafs_reserve_economics.json` | `SoraFSReserveLedgerTopUpRequired` `requires_top_up=1` qachon yonadi; `SoraFSReserveLedgerUnderwritingBreach` `meets_underwriting=0` qachon yonadi. |
| `sorafs_reserve_ledger_transfer_xor`, `sorafs_reserve_ledger_instruction_total` | `dashboards/grafana/sorafs_reserve_economics.json` da “Turga koʻra oʻtkazmalar”, “Oxirgi transferlar taqsimoti” va qamrov kartalari | `SoraFSReserveLedgerInstructionMissing`, `SoraFSReserveLedgerRentTransferMissing` va `SoraFSReserveLedgerTopUpTransferMissing` ijara/toʻldirish zarur boʻlsa ham transfer tasmasi yoʻq yoki nolga teng boʻlganda ogohlantiradi; qamrov kartalari xuddi shu holatlarda 0% ga tushadi. |

Ijaraga olish davri tugagach, Prometheus/NDJSON suratlarini yangilang, tasdiqlang
Grafana panellari yangi `label` ni oladi va ekran tasvirlarini biriktiradi +
Alertmanager identifikatorlari ijarani boshqarish paketiga. Bu CLI proektsiyasini tasdiqlaydi,
telemetriya va boshqaruv artefaktlarining barchasi **bir xil** uzatish tasmasidan va
yo'l xaritasining iqtisod panellarini Reserve + Rent bilan moslashtiradi
avtomatlashtirish. Qamrov kartalari 100% (yoki 1.0) va yangi ogohlantirishlarni o'qishi kerak
Ijara va zaxira to'ldirish o'tkazmalari dayjestda mavjud bo'lgandan keyin tozalanishi kerak.