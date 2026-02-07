---
lang: uz
direction: ltr
source: docs/examples/soranet_gateway_billing/reconciliation_report_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5c10cd7eda24260bfd1319c7b8ac23dba2a1c8a1cb39ea49f0f1a64427ca15db
source_last_modified: "2025-12-29T18:16:35.086260+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraGlobal Gateway hisob-kitoblarni muvofiqlashtirish

- **Oyna:** `<from>/<to>`
- **Ijarachi:** `<tenant-id>`
- **Katalog versiyasi:** `<catalog-version>`
- **Foydalanish surati:** `<path or hash>`
- **Qo'riqchilar:** yumshoq qopqoq `<soft-cap-xor> XOR`, qattiq qopqoq `<hard-cap-xor> XOR`, ogohlantirish chegarasi `<alert-threshold>%`
- **To'lovchi -> G'aznachilik:** `<payer>` -> `<treasury>`, `<asset-definition>`
- **Toʻlovning umumiy miqdori:** `<total-xor> XOR` (`<total-micros>` micro-XOR)

## Satr elementini tekshirish
- [ ] Foydalanish yozuvlari faqat katalog hisoblagich identifikatorlari va joriy hisob-kitob hududlarini qamrab oladi
- [ ] Miqdor birliklari katalog taʼriflariga mos keladi (soʻrovlar, GiB, ms va h.k.)
- [ ] Katalog bo'yicha qo'llaniladigan mintaqa ko'paytmalari va chegirma darajalari
- [ ] CSV/Parket eksporti JSON faktura satriga mos keladi

## Guardrail baholash
- [ ] Yumshoq qopqoq ogohlantirish chegarasiga yetib keldingizmi? `<yes/no>` (ha bo'lsa, ogohlantirish dalillarini ilova qiling)
- [ ] Qattiq qopqoq oshib ketdimi? `<yes/no>` (agar shunday bo'lsa, bekor qilishni tasdiqlashni ilova qiling)
- [ ] Hisob-fakturaning minimal darajasi qoniqtirildi

## Buxgalteriya proyeksiyasi
- [ ] O'tkazma to'plami jami hisob-fakturadagi `total_micros` ga teng
- [ ] Obyekt taʼrifi hisob-kitob valyutasiga mos keladi
- [ ] To'lovchi va g'azna hisoblari ijarachi va operatorga mos keladi
- [ ] Auditni takrorlash uchun biriktirilgan Norito/JSON artefaktlari

## Bahs/tuzatish bo'yicha eslatmalar
- Kuzatilgan farq: `<variance detail>`
- Taklif etilayotgan sozlash: `<delta and rationale>`
- Qo'llab-quvvatlovchi dalillar: `<logs/dashboards/alerts>`

## Tasdiqlashlar
- Billing tahlilchisi: `<name + signature>`
- G'aznachilik sharhlovchisi: `<name + signature>`
- Boshqaruv paketi xeshi: `<hash/reference>`