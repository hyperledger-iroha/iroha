---
lang: uz
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

# SoraNet GAR qabul qilish shabloni

GAR amalini talab qilganda ushbu qabul qilish shaklidan foydalaning (tozalash, ttl bekor qilish, tarif
shift, moderatsiya direktivasi, geofence yoki qonuniy ushlab turish). Taqdim etilgan shakl
audit jurnallari va `gar_controller` chiqishlari bilan birga biriktirilishi kerak.
kvitansiyalar xuddi shu dalil URIlarni keltiradi.

| Maydon | Qiymat | Eslatmalar |
|-------|-------|-------|
| So'rov identifikatori |  | Guardian/ops chipta identifikatori. |
| | tomonidan so'ragan  | Hisob + kontakt. |
| Sana/vaqt (UTC) |  | Harakat qachon boshlanishi kerak. |
| GAR nomi |  | masalan, `docs.sora`. |
| Kanonik xost |  | masalan, `docs.gw.sora.net`. |
| Harakat |  | `ttl_override` / `rate_limit_override` / `purge_static_zone` / `geo_fence` / `legal_hold` / `moderation`. |
| TTL bekor qilish (sekundlar) |  | Faqat `ttl_override` uchun talab qilinadi. |
| Tariflar chegarasi (RPS) |  | Faqat `rate_limit_override` uchun talab qilinadi. |
| Ruxsat berilgan hududlar |  | `geo_fence` so'rovida ISO mintaqalari ro'yxati. |
| Rad etilgan hududlar |  | `geo_fence` so'rovida ISO mintaqalar ro'yxati. |
| Moderatsiya slugs |  | GAR moderatsiya direktivasiga mos keling. |
| Teglarni tozalash |  | Xizmat qilishdan oldin tozalanishi kerak bo'lgan teglar. |
| Yorliqlar |  | Mashina yorliqlari (hodisalar identifikatori, matkap nomi, pop-scope). |
| Dalil URI'lari |  | So'rovni qo'llab-quvvatlovchi jurnallar/boshqaruv paneli/spets. |
| Audit URI |  | Standartlardan farqli bo'lsa, har bir pop audit URI. |
| Talab qilingan muddat |  | Unix vaqt tamg'asi yoki RFC3339; sukut bo'yicha bo'sh qoldiring. |
| Sabab |  | Foydalanuvchi bilan bog'liq tushuntirish; kvitansiyalar va asboblar panelida ko'rinadi. |
| Tasdiqlovchi |  | So'rov uchun qo'riqchi/qo'mita tasdiqlovchisi. |

### Yuborish bosqichlari

1. Jadvalni to'ldiring va uni boshqaruv chiptasiga ilova qiling.
2. GAR kontroller konfiguratsiyasini (`policies`/`pops`) mos ravishda yangilang
   `labels`/`evidence_uris`/`expires_at_unix`.
3. Voqealar/kvitansiyalarni chiqarish uchun `cargo xtask soranet-gar-controller ...` ni ishga tushiring.
4. `gar_controller_summary.json`, `gar_reconciliation_report.json`,
   `gar_metrics.prom` va `gar_audit_log.jsonl` bir xil chiptaga. The
   tasdiqlovchi kvitansiya soni jo‘natishdan oldin PoP ro‘yxatiga mos kelishini tasdiqlaydi.