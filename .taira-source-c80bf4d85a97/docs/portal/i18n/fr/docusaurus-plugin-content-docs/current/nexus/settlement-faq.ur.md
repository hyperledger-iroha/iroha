---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/settlement-faq.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : nexus-settlement-faq
titre : FAQ sur le règlement
description : Il s'agit d'un routage de règlement, d'une conversion XOR, d'un lien vers un routage de règlement, d'une conversion XOR.
---

FAQ sur le règlement des litiges (`docs/source/nexus_settlement_faq.md`) mono-repo میں تلاش کیے بغیر دیکھ سکیں۔ Un routeur de règlement est disponible pour un routeur de règlement. Il existe des SDK et des charges utiles Norito pour les SDK et les charges utiles Norito.

## نمایاں نکات

1. **voie میپنگ** — ہر dataspace ایک `settlement_handle` et `xor_global`, `xor_lane_weighted`, `xor_hosted_custody` یا `xor_dual_fund`)۔ `docs/source/project_tracker/nexus_config_deltas/` Mise à jour du catalogue de voies
2. **متعین تبدیلی** — routeur pour les règlements et la gouvernance et les sources de liquidités pour XOR et pour les sources de liquidité Il y a des voies et des tampons XOR et des tampons XOR. coupes de cheveux صرف تب لاگو ہوتے ہیں جب buffers پالیسی سے باہر جائیں۔
3. **ٹیلی میٹری** — `nexus_settlement_latency_seconds`, compteurs de conversion, et jauges de coupe de cheveux pour les utilisateurs tableaux de bord `dashboards/grafana/nexus_settlement.json` et alertes `dashboards/alerts/nexus_audit_rules.yml` et alertes
4. **ثبوت** — audits, configurations, journaux de routeur, exportations de télémétrie, rapports de rapprochement et rapports de rapprochement
5. ** SDK inclus ** — SDK pour les aides au règlement, les identifiants de voie et les encodeurs de charge utile Norito pour les routeurs et les routeurs. برابری رہے۔

## مثال کے بہاؤ| voie کی قسم | جمع کرنے والا ثبوت | یہ کیا ثابت کرتا ہے |
|-----------|----------|----------------|
| Produit `xor_hosted_custody` | journal du routeur + `nexus_settlement_latency_seconds{lane}` + `settlement_router_haircut_total{lane}` | Tampons CBDC pour XOR pour les coupes de cheveux et les coupes de cheveux |
| عوامی `xor_global` | journal du routeur + DEX/TWAP + métriques de latence/conversion | مشترکہ liquidité راستے نے منتقل شدہ رقم کو شائع شدہ TWAP پر zéro décote کے ساتھ قیمت دی۔ |
| ہائبرڈ `xor_dual_fund` | journal du routeur et compteurs publics blindés + compteurs de télémétrie | ratios de gouvernance protégés/publics |

## مزید تفصیل چاہیے؟

- FAQ suivante : `docs/source/nexus_settlement_faq.md`
- Spécifications du routeur de règlement : `docs/source/settlement_router.md`
- Playbook CBDC : `docs/source/cbdc_lane_playbook.md`
- Runbook d'opérations : [Opérations Nexus](./nexus-operations)