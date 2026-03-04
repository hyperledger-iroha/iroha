---
lang: az
direction: ltr
source: docs/examples/sns/arbitration_transparency_report_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 305a3f3b253a013825d4dd798d2282e111913ec777fe0fbf5b02a92c7172b92a
source_last_modified: "2025-12-29T18:16:35.076964+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->
# SNS Arbitraj Şəffaflıq Hesabatı — <Ay YYYY>

- **Şəkilçi:** `<.sora / .nexus / .dao>`
- **Hesabat pəncərəsi:** `<ISO start>` → `<ISO end>`
- **Hazırlayan:** `<Council liaison>`
- **Mənbə artefaktları:** `cases.ndjson` SHA256 `<hash>`, idarə paneli ixracı `<filename>.json`

## 1. Xülasə

- Ümumi yeni hallar: `<count>`
- Bu müddət ərzində bağlanan işlər: `<count>`
- SLA uyğunluğu: `<ack %>` təsdiq / `<resolution %>` qərarı
- Qəyyumun ləğvi buraxıldı: `<count>`
- Köçürmələr/geri qaytarılmalar yerinə yetirildi: `<count>`

## 2. Case Mix

| Mübahisə növü | Yeni hallar | Qapalı işlər | Median ayırdetmə (günlər) |
|-------------|-----------|--------------|--------------------------|
| Mülkiyyət | 0 | 0 | 0 |
| Siyasət pozuntusu | 0 | 0 | 0 |
| Sui-istifadə | 0 | 0 | 0 |
| Faturalandırma | 0 | 0 | 0 |
| Digər | 0 | 0 | 0 |

## 3. SLA Performansı

| Prioritet | SLA-nı qəbul edin | Əldə edildi | Qətnamə SLA | Əldə edildi | pozuntular |
|----------|-----------------|----------|----------------|----------|----------|
| Təcili | ≤ 2sa | 0% | ≤ 72 saat | 0% | 0 |
| Yüksək | ≤ 8 saat | 0% | ≤ 10g | 0% | 0 |
| Standart | ≤ 24 saat | 0% | ≤ 21g | 0% | 0 |
| Məlumat | ≤ 3d | 0% | ≤ 30d | 0% | 0 |

İstənilən pozuntuların əsas səbəblərini təsvir edin və təmir biletlərinə keçid edin.

## 4. İşin Qeydiyyatı

| Case ID | Seçici | Prioritet | Status | Nəticə | Qeydlər |
|---------|----------|----------|--------|---------|-------|
| SNS-YYYY-NNNNN | `label.suffix` | Standart | Bağlıdır | Saxlanıldı | `<summary>` |

Anonim faktlara və ya ictimai səsvermə keçidlərinə istinad edən bir sətirlik qeydlər təqdim edin. Möhür
tələb olunduqda və redaksiyanın tətbiq edildiyini qeyd edin.

## 5. Fəaliyyətlər və Müalicələr

- **Dondurur / buraxır:** `<counts + case ids>`
- **Köçürmələr:** `<counts + assets moved>`
- **Faktura düzəlişləri:** `<credits/debits>`
- **Siyasət təqibləri:** `<tickets or RFCs opened>`

## 6. Müraciətlər və Qəyyumun ləğv edilməsi

Qəyyumlar şurasına göndərilən müraciətləri, o cümlədən vaxt nişanları və
qərarlar (təsdiq / rədd). `sns governance appeal` qeydlərinə və ya şurasına keçid
səslər.

## 7. Görkəmli maddələr

- `<Action item>` — Sahib `<name>`, ETA `<date>`
- `<Action item>` — Sahib `<name>`, ETA `<date>`

Bu hesabatda istinad edilən NDJSON, Grafana ixracları və CLI jurnallarını əlavə edin.