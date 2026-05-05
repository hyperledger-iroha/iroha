---
lang: az
direction: ltr
source: docs/portal/docs/sns/suffix-catalog.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Sora Name Service Suffix Catalog
sidebar_label: Suffix catalog
description: Canonical allowlist of SNS suffixes, stewards, and pricing knobs for `.sora`, `.nexus`, and `.dao`.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Sora Name Service Suffix Catalog

SNS yol xəritəsi hər təsdiq edilmiş şəkilçini (SN-1/SN-2) izləyir. Bu səhifəni əks etdirir
registratorları, DNS şlüzlərini və ya pul kisəsini idarə edən operatorlar üçün həqiqət mənbəyi kataloqu
alətlər status sənədlərini silmədən eyni parametrləri yükləyə bilər.

- **Snapshot:** [`docs/examples/sns/suffix_catalog_v1.json`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/examples/sns/suffix_catalog_v1.json)
- **İstehlakçılar:** `iroha sns policy`, SNS onboarding dəstləri, KPI panelləri və
  DNS/Gateway buraxılış skriptlərinin hamısı eyni JSON paketini oxuyur.
- **Statuslar:** `active` (qeydiyyata icazə verilir), `paused` (müvəqqəti qapalı),
  `revoked` (elan edilib, lakin hazırda mövcud deyil).

## Kataloq sxemi

| Sahə | Növ | Təsvir |
|-------|------|-------------|
| `suffix` | simli | Baş nöqtəli insan tərəfindən oxuna bilən şəkilçi. |
| `suffix_id` | `u16` | `SuffixPolicyV1::suffix_id`-də kitabda saxlanılan identifikator. |
| `status` | enum | `active`, `paused` və ya `revoked` buraxılışa hazırlığı təsvir edir. |
| `steward_account` | simli | İdarəetmə üçün cavabdeh olan hesab (registrator siyasət qarmaqlarına uyğun gəlir). |
| `fund_splitter_account` | simli | `fee_split` üzrə marşrutlaşdırmadan əvvəl ödənişləri qəbul edən hesab. |
| `payment_asset_id` | simli | Hesablaşma üçün istifadə olunan aktiv (ilkin kohort üçün `61CtjvNd9T3THAR65GsMVHr82Bjc`). |
| `min_term_years` / `max_term_years` | tam | Siyasətdən satınalma müddətini məhdudlaşdırın. |
| `grace_period_days` / `redemption_period_days` | tam | Yeniləmə təhlükəsizlik pəncərələri Torii tərəfindən tətbiq edilir. |
| `referral_cap_bps` | tam | Rəhbərlik tərəfindən icazə verilən maksimum müraciətin ayrılması (əsas nöqtələr). |
| `reserved_labels` | massiv | İdarəetmə tərəfindən qorunan etiket obyektləri `{label, assigned_to, release_at_ms, note}`. |
| `pricing` | massiv | `label_regex`, `base_price`, `auction_kind` və müddət hədləri olan səviyyəli obyektlər. |
| `fee_split` | obyekt | `{treasury_bps, steward_bps, referral_max_bps, escrow_bps}` baza nöqtəsi bölünməsi. |
| `policy_version` | tam | İdarəetmə siyasəti redaktə etdikdə monoton sayğac artırılır. |

## Cari kataloq

| şəkilçi | ID (`hex`) | Stüard | Fond bölücü | Status | Ödəniş aktivi | İstinad həddi (bps) | Müddət (min – max illər) | Grace / Redemption (günlər) | Qiymətləndirmə səviyyələri (regex → əsas qiymət / auksion) | Qorunan etiketlər | Ödəniş bölgüsü (T/S/R/E bps) | Siyasət versiyası |
|--------|------------|---------|---------------|--------|---------------|--------------------|--------------------------|--------------------------|--------------------------|-----------------|
| `.sora` | `0x0001` | `<i105-account-id>` | `<i105-account-id>` | Aktiv | `61CtjvNd9T3THAR65GsMVHr82Bjc` | 500 | 1 – 5 | 30 / 60 | `T0: ^[a-z0-9]{3,}$ → 120 XOR (Vickrey)` | `treasury → <i105-account-id>` | `7000 / 3000 / 1000 / 0` | 1 |
| `.nexus` | `0x0002` | `<i105-account-id>` | `<i105-account-id>` | Dayandırıldı | `61CtjvNd9T3THAR65GsMVHr82Bjc` | 300 | 1 – 3 | 15 / 30 | `T0: ^[a-z0-9]{4,}$ → 480 XOR (Vickrey)`<br>`T1: ^[a-z]{2}$ → 4000 XOR (Dutch floor 500)` | `treasury → <i105-account-id>`, `guardian → <i105-account-id>` | `6500 / 2500 / 800 / 200` | 2 |
| `.dao` | `0x0003` | `<i105-account-id>` | `<i105-account-id>` | Ləğv edildi | `61CtjvNd9T3THAR65GsMVHr82Bjc` | 0 | 1 – 2 | 30 / 30 | `T0: ^[a-z0-9]{3,}$ → 60 XOR (Vickrey)` | `dao (held for future release)` | `9000 / 1000 / 0 / 0` | 0 |

## JSON çıxarışı

```json
{
  "version": 1,
  "generated_at": "2026-05-01T00:00:00Z",
  "suffixes": [
    {
      "suffix": ".sora",
      "suffix_id": 1,
      "status": "active",
      "fund_splitter_account": "<i105-account-id>",
      "payment_asset_id": "61CtjvNd9T3THAR65GsMVHr82Bjc",
      "referral_cap_bps": 500,
      "pricing": [
        {
          "tier_id": 0,
          "label_regex": "^[a-z0-9]{3,}$",
          "base_price": {"asset_id": "61CtjvNd9T3THAR65GsMVHr82Bjc", "amount": 120},
          "auction_kind": "vickrey_commit_reveal",
          "min_duration_years": 1,
          "max_duration_years": 5
        }
      ],
      "...": "see docs/examples/sns/suffix_catalog_v1.json for the full record"
    }
  ]
}
```

## Avtomatlaşdırma qeydləri

1. Operatorlara paylamazdan əvvəl JSON snapşotunu yükləyin və onu hash edin/imzalayın.
2. Qeydiyyatçı alətləri `suffix_id`, müddət məhdudiyyətləri və qiymətləri əks etdirməlidir.
   sorğu `/v1/sns/*`-ə çatdıqda kataloqdan.
3. DNS/Gateway köməkçiləri GAR yaradan zaman qorunan etiket metadatasını oxuyur
   şablonlar, beləliklə DNS cavabları idarəetmə nəzarətləri ilə uyğunlaşdırılır.
4. KPI əlavə iş etiketi tablosuna metadata şəkilçisi ilə ixrac edilir ki, xəbərdarlıqlar ilə uyğun gəlir
   işə salınma vəziyyəti burada qeydə alınıb.