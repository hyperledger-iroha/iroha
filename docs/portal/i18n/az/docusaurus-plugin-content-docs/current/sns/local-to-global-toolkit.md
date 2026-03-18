---
lang: az
direction: ltr
source: docs/portal/docs/sns/local-to-global-toolkit.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Local → Global Address Toolkit
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

Bu səhifəni əks etdirir [`docs/source/sns/local_to_global_toolkit.md`](../../../source/sns/local_to_global_toolkit.md)
mono-repodan. O, **ADDR-5c** yol xəritəsi elementi tərəfindən tələb olunan CLI köməkçilərini və runbook-ları paketləşdirir.

## Baxış

- `scripts/address_local_toolkit.sh`, istehsal etmək üçün `iroha` CLI-ni bağlayır:
  - `audit.json` — `iroha tools address audit --format json`-dən strukturlaşdırılmış çıxış.
  - `normalized.txt` — hər Yerli domen seçicisi üçün çevrilmiş üstünlük verilən I105 / ikinci ən yaxşı sıxılmış (`sora`) literalları.
- Skripti ünvan qəbulu tablosu ilə cütləşdirin (`dashboards/grafana/address_ingest.json`)
  və Yerli-8 / sübut etmək üçün Alertmanager qaydaları (`dashboards/alerts/address_ingest_rules.yml`)
  Yerli-12 kəsmə təhlükəsizdir. Yerli-8 və Yerli-12 toqquşma panellərinə və əlavə olaraq baxın
  `AddressLocal8Resurgence`, `AddressLocal12Collision` və `AddressInvalidRatioSlo` xəbərdarlıqları əvvəl
  açıq dəyişiklikləri təşviq edir.
- [Ünvan Ekranı Təlimatlarına](address-display-guidelines.md) və
  UX və insident-cavab konteksti üçün [Ünvan Manifest runbook](../../../source/runbooks/address_manifest_ops.md).

## İstifadəsi

```bash
scripts/address_local_toolkit.sh \
  --input fixtures/address/local_digest_examples.txt \
  --output-dir artifacts/address_migration \
  --network-prefix 753 \
  --format i105
```

Seçimlər:

- I105 əvəzinə `i105` çıxışı üçün `--format i105`.
- `domainless output (default)` çılpaq hərflər yaymaq üçün.
- Dönüşüm addımını atlamaq üçün `--audit-only`.
- `--allow-errors` xətalı sətirlər görünəndə skan etməyə davam etmək üçün (CLI davranışına uyğundur).

Skript işin sonunda artefakt yollarını yazır. Hər iki faylı əlavə edin
Sıfırı sübut edən Grafana ekran görüntüsü ilə birlikdə dəyişiklik idarəetmə biletiniz
≥30 gün ərzində yerli-8 aşkarlama və sıfır Yerli-12 toqquşma.

## CI inteqrasiyası

1. Skripti xüsusi bir işdə işə salın və onun nəticələrini yükləyin.
2. `audit.json` Yerli seçiciləri (`domain.kind = local12`) bildirdikdə blok birləşir.
   defolt `true` dəyərində (yalnız inkişaf/sınaq qruplarında `false`-ə keçin
   reqressiyaların diaqnostikası) və əlavə edin
   `iroha tools address normalize`-dən CI-yə belə reqressiya
   cəhdlər istehsala çatmazdan əvvəl uğursuz olur.

Ətraflı təfərrüatlar, nümunə sübut yoxlama siyahıları və müştərilərə kəsilməni elan edərkən təkrar istifadə edə biləcəyiniz buraxılış qeydi parçası üçün mənbə sənədinə baxın.