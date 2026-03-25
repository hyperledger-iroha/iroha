---
lang: az
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

Ehtiyat+Kirayə siyasəti (yol xəritəsi elementi **SFM‑6**) indi `sorafs reserve` göndərir
CLI köməkçiləri və `scripts/telemetry/reserve_ledger_digest.py` tərcüməçisi belədir
xəzinə əməliyyatları deterministik icarə/ehtiyat köçürmələri buraxa bilər. Bu səhifəni əks etdirir
`docs/source/sorafs_reserve_rent_plan.md`-də müəyyən edilmiş iş axını və izah edir
yeni transfer lentini Grafana + Alertmanager-ə necə bağlamaq olar ki, iqtisadiyyat və
idarəetməni nəzərdən keçirənlər hər hesablaşma dövrünü yoxlaya bilərlər.

## Başdan sona iş axını

1. **Sitat + kitab proyeksiyası**
   ```bash
   sorafs reserve quote \
     --storage-class hot \
     --tier tier-a \
     --duration monthly \
     --gib 250 \
     --quote-out artifacts/sorafs_reserve/quotes/provider-alpha-apr.json

  sorafs reserve ledger \
    --quote artifacts/sorafs_reserve/quotes/provider-alpha-apr.json \
    --provider-account i105... \
    --treasury-account i105... \
    --reserve-account i105... \
    --asset-definition 61CtjvNd9T3THAR65GsMVHr82Bjc \
    --json-out artifacts/sorafs_reserve/ledger/provider-alpha-apr.json
   ```
   Defter köməkçisi `ledger_projection` blokunu əlavə edir (icarə haqqı, ehtiyat
   çatışmazlıq, əlavə delta, anderraytinq booleanları) üstəgəl Norito `Transfer`
   ISI-lər XOR-u xəzinədarlıq və ehtiyat hesabları arasında köçürmək üçün lazım idi.

2. **Dəzim + Prometheus/NDJSON çıxışlarını yaradın**
   ```bash
   python3 scripts/telemetry/reserve_ledger_digest.py \
     --ledger artifacts/sorafs_reserve/ledger/provider-alpha-apr.json \
     --label provider-alpha-apr \
     --out-json artifacts/sorafs_reserve/ledger/provider-alpha-apr.digest.json \
     --out-md docs/source/sorafs/reports/provider-alpha-apr-ledger.md \
     --ndjson-out artifacts/sorafs_reserve/ledger/provider-alpha-apr.ndjson \
     --out-prom artifacts/sorafs_reserve/ledger/provider-alpha-apr.prom
   ```
   Həzm köməkçisi mikro-XOR cəmlərini XOR-a normallaşdırır, olub olmadığını qeyd edir
   proyeksiya anderraytinqə cavab verir və **köçürmə lenti** göstəricilərini yayır
   `sorafs_reserve_ledger_transfer_xor` və
   `sorafs_reserve_ledger_instruction_total`. Çoxlu dəftərlərin olması lazım olduqda
   işlənmiş (məsələn, provayderlər toplusu), `--ledger`/`--label` cütlərini təkrarlayın və
   köməkçi hər həzmdən ibarət tək NDJSON/Prometheus faylı yazır.
   tablosuna sifarişli yapışqan olmadan bütün dövrü qəbul edir. `--out-prom`
   fayl node-exporter mətn faylı kollektorunu hədəfləyir - `.prom` faylını daxil edin
   ixracatçının baxılan kataloqunu və ya onu telemetriya qutusuna yükləyin
   Ehtiyat tablosunun işi tərəfindən istehlak edilir - `--ndjson-out` eyni şəkildə qidalanır
   məlumat boru kəmərlərinə faydalı yüklər.

3. **Artefaktları + sübutları dərc edin**
   - Həzmləri `artifacts/sorafs_reserve/ledger/<provider>/` altında saxlayın və əlaqə saxlayın
     həftəlik iqtisadiyyat hesabatınızdan Markdown xülasəsi.
   - JSON həzmini icarə haqqının yandırılmasına əlavə edin (belə ki, auditorlar
     riyaziyyat) və yoxlama məbləğini idarəetmə sübut paketinə daxil edin.
   - Əgər həzm əlavə və ya anderraytinq pozuntusu barədə siqnal verirsə, xəbərdarlığa istinad edin
     ID-lər (`SoraFSReserveLedgerTopUpRequired`,
     `SoraFSReserveLedgerUnderwritingBreach`) və ISI-lərin hansı transfer olduğunu qeyd edin
     tətbiq edilir.

## Metriklər → tablolar → xəbərdarlıqlar

| Mənbə metrik | Grafana paneli | Xəbərdarlıq / siyasət çəngəl | Qeydlər |
|-------------|---------------|---------------------|-------|
| `torii_da_rent_base_micro_total`, `torii_da_protocol_reserve_micro_total`, `torii_da_provider_reward_micro_total` | `dashboards/grafana/sorafs_capacity_health.json`-də “DA Rent Distribution (XOR/saat)” | Həftəlik xəzinə həzmini qidalandırın; ehtiyat axınındakı sıçrayışlar `SoraFSCapacityPressure` (`dashboards/alerts/sorafs_capacity_rules.yml`) daxilində yayılır. |
| `torii_da_rent_gib_months_total` | “Tutuum İstifadəsi (GiB-aylar)” (eyni idarə paneli) | Fakturalı yaddaşın XOR köçürmələrinə uyğun olduğunu sübut etmək üçün mühasibat kitabçası ilə birləşdirin. |
| `sorafs_reserve_ledger_rent_due_xor`, `sorafs_reserve_ledger_reserve_shortfall_xor`, `sorafs_reserve_ledger_top_up_shortfall_xor` | “Reserve Snapshot (XOR)” + status kartları `dashboards/grafana/sorafs_reserve_economics.json` | `SoraFSReserveLedgerTopUpRequired`, `requires_top_up=1` zaman atəş edir; `SoraFSReserveLedgerUnderwritingBreach`, `meets_underwriting=0` zaman işə düşür. |
| `sorafs_reserve_ledger_transfer_xor`, `sorafs_reserve_ledger_instruction_total` | `dashboards/grafana/sorafs_reserve_economics.json`-də “Növə görə köçürmələr”, “Son Köçürmə Dağılımı” və əhatə kartları | `SoraFSReserveLedgerInstructionMissing`, `SoraFSReserveLedgerRentTransferMissing` və `SoraFSReserveLedgerTopUpTransferMissing` icarəyə/yükləmə tələb olunsa da, köçürmə lenti olmadıqda və ya sıfır olduqda xəbərdarlıq edir; əhatə kartları eyni hallarda 0%-ə düşür. |

İcarəyə götürmə dövrü başa çatdıqda, Prometheus/NDJSON snapşotlarını yeniləyin, təsdiqləyin
Grafana panelləri yeni `label`-i götürür və ekran görüntülərini əlavə edir +
İcarəyə götürmə paketi üçün Alertmanager ID'ləri. Bu, CLI proyeksiyasını sübut edir,
telemetriya və idarəetmə artefaktlarının hamısı **eyni** transfer lentindən və
yol xəritəsinin iqtisadiyyat panellərini Ehtiyat+Kirayə ilə uyğunlaşdırır
avtomatlaşdırma. Əhatə kartları 100% (və ya 1.0) və yeni xəbərdarlıqları oxumalıdır
Kirayə və ehtiyat əlavə köçürmələr həzmdə mövcud olduqdan sonra təmizlənməlidir.