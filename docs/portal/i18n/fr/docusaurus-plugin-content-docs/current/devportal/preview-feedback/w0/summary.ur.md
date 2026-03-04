---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w0/summary.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : aperçu-feedback-w0-summary
titre : W0 کے وسط کا فيڈبیک خلاصہ
sidebar_label : W0 فيڈبیک (وسط)
description : les mainteneurs principaux ont une vague de prévisualisation et des éléments d'action.
---

| آئٹم | تفصیل |
| --- | --- |
| لہر | W0 - mainteneurs principaux |
| خلاصہ تاریخ | 2025-03-27 |
| ریویو ونڈو | 2025-03-25 -> 2025-04-08 |
| شرکا | docs-core-01, sdk-rust-01, sdk-js-01, sorafs-ops-01, observabilité-01 |
| آرٹیفیکٹ ٹیگ | `preview-2025-03-24` |

## نمایاں نکات

1. **Checksum ورک فلو** - Les réviseurs de ce groupe ont trouvé `scripts/preview_verify.sh`
   مشترکہ descriptor/archive جوڑی کے خلاف کامیاب رہا۔ کسی دستی override کی
   ضرورت نہیں ہوئی۔
2. **نیویگیشن فيڈبیک** - sidebar کی ترتیب میں دو معمولی مسائل رپورٹ ہوئے
   (`docs-preview/w0 #1-#2`). دونوں Docs/DevRel کو دیے گئے اور لہر کو بلاک
   نہیں کرتے۔
3. ** Runbook SoraFS disponible ** - sorafs-ops-01 par `sorafs/orchestrator-ops` ici
   `sorafs/multi-source-rollout` liens croisés et liens croisés
   problème de suivi بنایا گیا؛ W1 سے پہلے حل کرنا ہے۔
4. **ٹیلیمیٹری ریویو** - observabilité-01 نے تصدیق کی کہ `docs.preview.integrity`,
   `TryItProxyErrors` et Journaux proxy Try-it vert vert کوئی alert فائر نہیں ہوا۔

## ایکشن آئٹمز| ID | وضاحت | مالک | اسٹیٹس |
| --- | --- | --- | --- |
| W0-A1 | Entrées de la barre latérale de devportal contiennent des critiques et des documents publiés (`preview-invite-*` pour les critiques). | Docs-core-01 | مکمل - barre latérale des documents des réviseurs کو مسلسل دکھاتا ہے (`docs/portal/sidebars.js`). |
| W0-A2 | `sorafs/orchestrator-ops` et `sorafs/multi-source-rollout` pour les liaisons croisées | Sorafs-ops-01 | مکمل - Le runbook est en cours de déploiement et le déploiement du dossier de configuration est terminé. |
| W0-A3 | outil de suivi de la gouvernance avec instantanés de télémétrie + ensemble de requêtes | Observabilité-01 | مکمل - bundle `DOCS-SORA-Preview-W0` کے ساتھ منسلک ہے۔ |

## اختتامی خلاصہ (2025-04-08)

- Les critiques de plusieurs critiques ont déjà analysé les builds et l'aperçu des versions.
  باہر نکل گئے؛ révocations d'accès `DOCS-SORA-Preview-W0` میں ریکارڈ ہیں۔
- لہر کے دوران کوئی incidents et alertes نہیں ہوئے؛ tableaux de bord de télémétrie پوری مدت
  vert رہے۔
- نیویگیشن + cross-link اقدامات (W0-A1/A2) نافذ ہو چکے ہیں اور اوپر کے docs میں
  دکھائی دیتے ہیں؛ traqueur de preuves télémétriques (W0-A3)
- ensemble de preuves contenant des captures d'écran de télémétrie, ainsi que des remerciements
  اور یہ خلاصہ problème de tracker سے لنک ہیں۔

## اگلے اقدامات

- Éléments d'action W1 et W0
- L'emplacement de préparation du proxy est actuellement disponible pour [flux d'invitation d'aperçu] (../../preview-invite-flow.md) میں
  Pour le contrôle en amont de la vague partenaire, cliquez ici_یہ خلاصہ [aperçu des invitations] (../../preview-invite-tracker.md) سے منسلک ہے تاکہ
Feuille de route DOCS-SORA en anglais_