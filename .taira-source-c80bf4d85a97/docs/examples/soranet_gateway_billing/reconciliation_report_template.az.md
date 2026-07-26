---
lang: az
direction: ltr
source: docs/examples/soranet_gateway_billing/reconciliation_report_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5c10cd7eda24260bfd1319c7b8ac23dba2a1c8a1cb39ea49f0f1a64427ca15db
source_last_modified: "2025-12-29T18:16:35.086260+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraGlobal Gateway Billinq uzlaşması

- **Pəncərə:** `<from>/<to>`
- **Kirayəçi:** `<tenant-id>`
- **Kataloq Versiyası:** `<catalog-version>`
- **İstifadə Snapshot:** `<path or hash>`
- **Qorxuluklar:** yumşaq qapaq `<soft-cap-xor> XOR`, sərt qapaq `<hard-cap-xor> XOR`, xəbərdarlıq həddi `<alert-threshold>%`
- **Ödəyici -> Xəzinədarlıq:** `<payer>` -> `<treasury>`, `<asset-definition>`
- **Ümumi ödəniş:** `<total-xor> XOR` (`<total-micros>` micro-XOR)

## Sətir elementinin yoxlanılması
- [ ] İstifadə qeydləri yalnız kataloq sayğac identifikatorlarını və etibarlı hesablaşma bölgələrini əhatə edir
- [ ] Kəmiyyət vahidləri kataloq təriflərinə uyğun gəlir (sorğular, GiB, ms və s.)
- [ ] Kataloq üzrə tətbiq olunan region çarpanları və endirim dərəcələri
- [ ] CSV/Parket ixracları JSON faktura sətir elementlərinə uyğun gəlir

## Korpusun Qiymətləndirilməsi
- [ ] Yumşaq qapaq siqnalı həddi çatdı? `<yes/no>` (əgər varsa, xəbərdarlıq sübutunu əlavə edin)
- [ ] Sərt qapaq keçildi? `<yes/no>` (əgər varsa, ləğvetmə təsdiqini əlavə edin)
- [ ] Minimum faktura mərtəbəsi razıdır

## Ledger Proyeksiyası
- [ ] Köçürmə partiyasının cəmi fakturada `total_micros`-ə bərabərdir
- [ ] Aktiv tərifi faktura valyutasına uyğun gəlir
- [ ] Ödəyici və xəzinə hesabları kirayəçi və operatorla uyğun gəlir
- [ ] Norito/JSON artefaktları auditin təkrarı üçün əlavə edilib

## Mübahisə/Tənzimləmə Qeydləri
- Müşahidə olunan fərq: `<variance detail>`
- Təklif olunan düzəliş: `<delta and rationale>`
- Dəstəkləyici sübut: `<logs/dashboards/alerts>`

## Təsdiqlər
- Ödəniş analitiki: `<name + signature>`
- Xəzinədarlığın rəyçisi: `<name + signature>`
- İdarəetmə paketi hash: `<hash/reference>`