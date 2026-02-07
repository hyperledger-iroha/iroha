---
lang: az
direction: ltr
source: docs/examples/soranet_gar_intake_form.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6cd4da7e590d581719ed2607994d7d9eb16d153fbd06f85655d0da37c727853a
source_last_modified: "2025-12-29T18:16:35.085419+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# SoraNet GAR Qəbul Şablonu

GAR əməliyyatı tələb edərkən bu qəbul formasından istifadə edin (təmizləmə, ttl ləğvi, dərəcə
tavan, moderasiya direktivi, coğrafi sərhəd və ya qanuni saxlama). Təqdim edilmiş forma
`gar_controller` çıxışları ilə yanaşı bağlanmalıdır ki, audit jurnalları və
qəbzlər eyni sübut URI-lərə istinad edir.

| Sahə | Dəyər | Qeydlər |
|-------|-------|-------|
| Sorğu ID |  | Qəyyum/operativ bilet id. |
| | tərəfindən tələb olunur  | Hesab + əlaqə. |
| Tarix/vaxt (UTC) |  | Fəaliyyət nə vaxt başlamalıdır. |
| GAR adı |  | məsələn, `docs.sora`. |
| Kanonik host |  | məsələn, `docs.gw.sora.net`. |
| Fəaliyyət |  | `ttl_override` / `rate_limit_override` / `purge_static_zone` / `geo_fence` / `legal_hold` / `moderation`. |
| TTL ləğvi (saniyələr) |  | Yalnız `ttl_override` üçün tələb olunur. |
| Tarif tavanı (RPS) |  | Yalnız `rate_limit_override` üçün tələb olunur. |
| İcazə verilən bölgələr |  | `geo_fence` sorğusu zamanı ISO region siyahısı. |
| İnkar edilən bölgələr |  | `geo_fence` sorğusu zamanı ISO region siyahısı. |
| Moderasiya şlakları |  | GAR moderasiya direktivlərini uyğunlaşdırın. |
| Teqləri təmizləyin |  | Xidmət vermədən əvvəl təmizlənməli olan etiketlər. |
| Etiketlər |  | Maşın etiketləri (hadisə id, qazma adı, pop əhatə dairəsi). |
| Sübut URI-ləri |  | Sorğunu dəstəkləyən qeydlər / tablolar / xüsusiyyətlər. |
| Audit URI |  | Defoltlardan fərqlidirsə, hər bir pop audit URI. |
| Sorğu müddəti |  | Unix vaxt damğası və ya RFC3339; default olaraq boş buraxın. |
| Səbəb |  | İstifadəçi ilə bağlı izahat; qəbzlərdə və tablosunda görünür. |
| Təsdiq edən |  | Sorğu üçün qəyyum/komitə təsdiqləyicisi. |

### Təqdimat addımları

1. Cədvəli doldurun və onu idarəetmə biletinə əlavə edin.
2. GAR nəzarətçi konfiqurasiyasını (`policies`/`pops`) uyğunluğu ilə yeniləyin
   `labels`/`evidence_uris`/`expires_at_unix`.
3. Hadisələri/qəbzləri yaymaq üçün `cargo xtask soranet-gar-controller ...`-i işə salın.
4. `gar_controller_summary.json`, `gar_reconciliation_report.json`,
   `gar_metrics.prom` və `gar_audit_log.jsonl` eyni biletə. The
   təsdiqləyici qəbz sayının göndərilməzdən əvvəl PoP siyahısına uyğun olduğunu təsdiqləyir.