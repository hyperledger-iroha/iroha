---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/settlement-faq.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : nexus-settlement-faq
titre : الأسئلة الشائعة للتسوية
description: إجابات موجهة للمشغلين تغطي توجيه التسوية وتحويل XOR والقياس عن بعد وأدلة التدقيق.
---

تعكس هذه الصفحة الأسئلة الشائعة الداخلية للتسوية (`docs/source/nexus_settlement_faq.md`) pour que les gens s'en servent. Il s'agit d'une solution de mono-repo. Vous pouvez également utiliser Settlement Router et utiliser le SDK pour vos besoins. Norito.

## أبرز النقاط

1. **Voie de transfert** — se trouve dans l'espace de données par `settlement_handle` (`xor_global` et `xor_lane_weighted` et `xor_hosted_custody` et `xor_dual_fund`). راجع أحدث كتالوج lane تحت `docs/source/project_tracker/nexus_config_deltas/`.
2. **تحويل حتمي** — يحول الـ router جميع التسويات إلى XOR عبر مصادر السيولة المعتمدة من الحوكمة. تقوم Lanes الخاصة بتمويل مخازن XOR مسبقا؛ Il y a aussi des coupes de cheveux qui sont aussi des choses à faire.
3. **القياس عن بعد** — راقب `nexus_settlement_latency_seconds` وعدادات التحويل ومقاييس coupe de cheveux. Il s'agit d'une application pour `dashboards/grafana/nexus_settlement.json` et d'une application pour `dashboards/alerts/nexus_audit_rules.yml`.
4. **الأدلة** — Sélectionnez les paramètres du routeur et les paramètres du routeur pour les configurer.
5. **مسؤوليات SDK** — يجب على كل SDK توفير أدوات مساعدة للتسوية ومعرفات lane et حمولات Norito Utilisez le routeur pour vous connecter.

## أمثلة على التدفقات| nouvelle voie | الأدلة المطلوبة | ماذا يثبت |
|-----------|----------|----------------|
| خاصة `xor_hosted_custody` | Acheter routeur + `nexus_settlement_latency_seconds{lane}` + `settlement_router_haircut_total{lane}` | Il y a aussi la CBDC comme XOR et les coupes de cheveux. |
| عامة `xor_global` | Ajouter un routeur + un DEX/TWAP + un routeur | Il s'agit d'une coupe de cheveux de TWAP. |
| هجينة `xor_dual_fund` | سجل الـ router يظهر تقسيم public مقابل blindé + عدادات القياس عن بعد | يثبت أن المزج بين blindé/public احترم نسب الحوكمة وسجل coupe de cheveux المطبق على كل جزء. |

## هل تحتاج مزيدا من التفاصيل؟

- FAQ Rubrique : `docs/source/nexus_settlement_faq.md`
- مواصفة Routeur de règlement : `docs/source/settlement_router.md`
- Banque CBDC : `docs/source/cbdc_lane_playbook.md`
- Nom du produit : [عمليات Nexus](./nexus-operations)